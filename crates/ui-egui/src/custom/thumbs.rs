//! First-page thumbnails for the Home document cards: rendered on a worker thread, cached as
//! egui textures. Read-only and local; nothing here touches or rewrites a document.
//!
//! CUSTOM(kelongyan): part of the fork's Home (custom/home.rs); see Rule/fork-customization.md.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use pdfcraft_render::{PageRenderer, Pixels, RenderConfig, RenderRequest, RequestKind};

/// Device pixels per PDF point for the thumbnail raster (a card is ~112 pt wide).
const THUMB_SCALE: f32 = 0.5;
/// A thumbnail source file larger than this is not read (untrusted input).
const MAX_BYTES: u64 = 256 << 20;
/// Most thumbnails kept per app; beyond this the cards keep their fallback face.
const MAX_READY: usize = 256;

/// One finished render, sent from the worker thread.
struct Rendered {
    path: String,
    width: u32,
    height: u32,
    rgba: Pixels,
}

#[derive(Default)]
struct Inner {
    worker: Option<Sender<String>>,
    results: Option<Receiver<Rendered>>,
    requested: HashSet<String>,
    failed: HashSet<String>,
    ready: HashMap<String, egui::TextureHandle>,
}

/// First-page thumbnails for document paths, rendered once each on a worker thread.
#[derive(Default)]
pub struct Thumbnails {
    inner: Mutex<Inner>,
}

impl Thumbnails {
    /// The first-page texture for `path`, requesting the render the first time it is asked for.
    /// `None` while it renders, when it cannot be rendered, or once the cache cap is reached.
    pub fn texture(&self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if path.is_empty() {
            return None;
        }
        // Adopt finished renders first, outside the lock: egui takes its own locks in
        // `load_texture`.
        let done: Vec<Rendered> = {
            let inner = lock(&self.inner);
            let mut done = Vec::new();
            if let Some(results) = &inner.results {
                while let Ok(r) = results.try_recv() {
                    done.push(r);
                }
            }
            done
        };
        for r in done {
            let image = egui::ColorImage::from_rgba_premultiplied([r.width as usize, r.height as usize], &r.rgba);
            let texture = ctx.load_texture(format!("home-thumb:{}", r.path), image, egui::TextureOptions::LINEAR);
            let mut inner = lock(&self.inner);
            if inner.ready.len() < MAX_READY {
                inner.requested.remove(&r.path);
                inner.ready.insert(r.path, texture);
            }
        }
        let mut inner = lock(&self.inner);
        if let Some(texture) = inner.ready.get(path) {
            return Some(texture.clone());
        }
        if inner.failed.contains(path) || inner.requested.contains(path) {
            return None;
        }
        if inner.worker.is_none() {
            let worker = spawn_worker()?;
            inner.worker = Some(worker.tasks);
            inner.results = Some(worker.results);
        }
        if let Some(tasks) = &inner.worker {
            if tasks.send(path.to_string()).is_ok() {
                inner.requested.insert(path.to_string());
            } else {
                inner.failed.insert(path.to_string());
            }
        }
        None
    }

    /// Whether the first-page texture for `path` is loaded (tests and review shots).
    pub fn is_ready(&self, path: &str) -> bool {
        lock(&self.inner).ready.contains_key(path)
    }
}

struct Worker {
    tasks: Sender<String>,
    results: Receiver<Rendered>,
}

/// Start the one render thread, or `None` when the thread cannot be created (the cards then
/// keep their fallback face).
fn spawn_worker() -> Option<Worker> {
    let (tasks_tx, tasks_rx) = std::sync::mpsc::channel::<String>();
    let (results_tx, results_rx) = std::sync::mpsc::channel::<Rendered>();
    std::thread::Builder::new()
        .name("home-thumbs".into())
        .stack_size(4 << 20)
        .spawn(move || {
            while let Ok(path) = tasks_rx.recv() {
                // A panic must cost one card, not the thread (and never the app).
                let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_first_page(&path))).unwrap_or(None);
                if let Some(r) = rendered
                    && results_tx.send(r).is_err()
                {
                    break;
                }
            }
        })
        .ok()?;
    Some(Worker { tasks: tasks_tx, results: results_rx })
}

/// Render page 0 of the PDF at `path` small. `None` for a missing, oversized, unreadable or
/// unrenderable file; the card then keeps its fallback face.
fn render_first_page(path: &str) -> Option<Rendered> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let mut renderer = PageRenderer::new(Arc::new(bytes), RenderConfig::default());
    if renderer.page_count() == 0 {
        return None;
    }
    let out = renderer.render(RenderRequest { page: 0, kind: RequestKind::Pixels, tile: None, scale: THUMB_SCALE, tag: 0, preview: true });
    if out.error.is_some() || out.rgba.is_empty() {
        return None;
    }
    Some(Rendered { path: path.to_string(), width: out.width, height: out.height, rgba: out.rgba })
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned lock still holds valid data; the cards must never crash over it.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_paths_render_nothing() {
        assert!(render_first_page("definitely/not/here.pdf").is_none());
        let junk = std::env::temp_dir().join(format!("pdfcraft-thumb-junk-{}.pdf", std::process::id()));
        std::fs::write(&junk, b"not a pdf at all").unwrap();
        assert!(render_first_page(&junk.to_string_lossy()).is_none());
        let _ = std::fs::remove_file(&junk);
    }
}
