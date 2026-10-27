//! CUSTOM(kelongyan): a side panel rises over the fork's motion ladder
//! (`crates/ui-egui/src/custom/tempo.rs`, rung `LARGE`). Local rule: Rule/fork-customization.md.
//!
//! Every test here renders to pixels, so each takes the test binary's shared GPU lock
//! (`crate::gpu()`, declared in `tests/it/main.rs`).

use egui::Rect;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::PdfCraftApp;

const PDF: &[u8] = b"%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] >> endobj
trailer << /Root 1 0 R >>
%%EOF";

fn harness() -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("doc.pdf", None, PDF.to_vec()).expect("opens");
        app.left_open = true;
        app
    });
    h.run_steps(4);
    h
}

/// Run a frame whose clock reads `at`. The harness holds its time still unless a test says
/// otherwise, which is also how `control.rs` drives frame-rate-dependent motion.
fn tick(h: &mut Harness<'static, PdfCraftApp>, at: f64) {
    h.input_mut().time = Some(at);
    h.step();
}

/// Where a tool row is drawn.
fn row_rect(h: &Harness<'static, PdfCraftApp>, label: &str) -> Rect {
    h.get_all_by_label(label).map(|n| n.rect()).next().unwrap_or_else(|| panic!("no row {label:?}"))
}

/// How much ink `rect` holds in the rendered frame, as dark pixels.
///
/// Measured off pixels rather than off the shape list because a fade is not a shape: egui
/// multiplies the opacity of everything a surface draws, so a row's colour in the list stays the
/// theme's and only the pixels it becomes grow lighter. Pixels are also what the user sees.
fn ink(h: &mut Harness<'static, PdfCraftApp>, rect: Rect) -> usize {
    let pixels = h.render().expect("the window renders");
    let (x0, y0) = (rect.min.x.max(0.0) as u32, rect.min.y.max(0.0) as u32);
    let (x1, y1) = (rect.max.x as u32, rect.max.y as u32);
    (y0..y1.min(pixels.height()))
        .flat_map(|y| (x0..x1.min(pixels.width())).map(move |x| (x, y)))
        .filter(|(x, y)| pixels.get_pixel(*x, *y).0[..3].iter().all(|v| *v < 90))
        .count()
}

/// A window that does not ease sees an opened panel exactly as it did before the ladder existed.
#[test]
fn a_still_window_shows_an_opened_panel_at_once() {
    let _gpu = crate::gpu();
    let mut h = harness();
    assert_eq!(h.ctx.global_style().animation_time, 0.0, "the harness asks for no easing");
    let row = row_rect(&h, "Export a PDF");
    let settled = ink(&mut h, row);
    h.state_mut().left_open = false;
    h.run_steps(3);
    h.state_mut().left_open = true;
    h.run_steps(3);
    let reopened = row_rect(&h, "Export a PDF");
    let again = ink(&mut h, reopened);
    assert!(settled > 20, "a panel shows its rows: {settled} dark pixels");
    assert_eq!(again, settled, "opening it again shows the same thing, at once");
}

/// Opening a panel brings its contents up over the workspace rung, and they arrive.
#[test]
fn a_panel_rises_when_it_opens() {
    let _gpu = crate::gpu();
    let mut h = harness();
    // A clock longer than any rung on the ladder, so no pass here can be mistaken for an arrival.
    h.ctx.all_styles_mut(|style| style.animation_time = 0.4);
    tick(&mut h, 300.0);
    let row = row_rect(&h, "Export a PDF");
    let whole = ink(&mut h, row);
    assert!(whole > 20, "at rest the panel is whole: {whole} dark pixels");

    // Close it and let it leave: the ladder is a timeline, so a panel reopened in the same pass it
    // was closed was never gone, and has nothing to rise from.
    h.state_mut().left_open = false;
    tick(&mut h, 300.1);
    tick(&mut h, 300.3);
    tick(&mut h, 300.6);

    h.state_mut().left_open = true;
    tick(&mut h, 300.8);
    let opening = ink(&mut h, row);
    assert!(opening < whole, "the pass the panel opens shows it on its way in: {opening} of {whole}");
    tick(&mut h, 300.9);
    let midway = ink(&mut h, row);
    assert!(midway > opening && midway < whole, "rising, not jumped: {opening} then {midway} then {whole}");
    // Past `LARGE.enter` from the pass it started: arrived, and whole again.
    tick(&mut h, 301.2);
    assert_eq!(ink(&mut h, row), whole, "arrived");
}

/// Re-opening the same panel rises again. This is why the travel is asked once a frame for every
/// panel by `panels::side_panels::show`, rather than from inside the panel's own draw: a closed panel is not
/// drawn at all, so a travel owned by the drawing would forget the closing and open flat.
#[test]
fn a_panel_rises_again_the_second_time() {
    let _gpu = crate::gpu();
    let mut h = harness();
    h.ctx.all_styles_mut(|style| style.animation_time = 0.4);
    tick(&mut h, 400.0);
    let row = row_rect(&h, "Export a PDF");
    let whole = ink(&mut h, row);
    for round in 0..2 {
        let base = 400.0 + f64::from(round) * 4.0;
        h.state_mut().left_open = false;
        for step in 1..=4 {
            tick(&mut h, base + f64::from(step) * 0.1);
        }
        h.state_mut().left_open = true;
        tick(&mut h, base + 0.8);
        let opening = ink(&mut h, row);
        assert!(opening < whole, "round {round} still rises: {opening} of {whole}");
        tick(&mut h, base + 1.6);
        assert_eq!(ink(&mut h, row), whole, "round {round} arrived");
    }
}
