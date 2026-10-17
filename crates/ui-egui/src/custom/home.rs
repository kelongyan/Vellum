//! This fork's Home view: the document library — a GoodNotes-style shelf of user-created folders
//! and the PDFs the app has opened. References only: nothing here copies, moves or deletes a file.
//!
//! CUSTOM(kelongyan): whole-file customization; the upstream layout it replaces is forwarded
//! from `../home.rs`. See `Rule/fork-customization.md` (local only).

use std::time::{Duration, UNIX_EPOCH};

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, vec2};

use crate::custom::library::{LibraryDoc, LibraryFolder};
use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, icons, widgets};

/// Card metrics of the grid.
const CARD_W: f32 = 170.0;
const CARD_GAP: f32 = 22.0;
const FOLDER_CARD_H: f32 = 160.0;
const DOC_CARD_H: f32 = 212.0;
/// Home stays this wide and centered on a large window, so the page keeps one calm column.
const CONTENT_WIDTH: f32 = 1040.0;

/// A starred folder's star.
const STAR: Color32 = Color32::from_rgb(0xE2, 0xA8, 0x2E);

/// The open modal, if any. Stored in egui's per-context data, like other transient UI state.
const DIALOG_ID: &str = "custom-home-dialog";
const DIALOG_TEXT_ID: &str = "custom-home-dialog-text";
/// Which folder is open; `None` (or a stale id) means the "All" top level.
const OPEN_FOLDER_ID: &str = "custom-home-open-folder";

#[derive(Clone, PartialEq)]
enum HomeDialog {
    NewFolder,
    RenameFolder(u64),
}

/// Everything the cards asked for, applied after the frame they were drawn in.
#[derive(Default)]
struct CardActions {
    open_folder: Option<Option<u64>>,
    open_doc: Option<String>,
    star: Option<(u64, bool)>,
    delete_folder: Option<u64>,
    rename_folder: Option<u64>,
    move_doc: Option<(String, Option<u64>)>,
    remove_doc: Option<String>,
    relocate_doc: Option<String>,
    open_dialog: Option<HomeDialog>,
}

pub(crate) fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let mut open_folder = ui.data_mut(|d| d.get_temp::<Option<u64>>(egui::Id::new(OPEN_FOLDER_ID)).flatten());
    if open_folder.is_some_and(|id| app.library.folder(id).is_none()) {
        open_folder = None;
        ui.data_mut(|d| d.insert_temp(egui::Id::new(OPEN_FOLDER_ID), open_folder));
    }
    let dialog = ui.data_mut(|d| d.get_temp::<Option<HomeDialog>>(egui::Id::new(DIALOG_ID))).flatten();

    let mut actions = CardActions::default();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 30, bottom: 24 }).show(ui, |ui| {
            let width = ui.available_width().min(CONTENT_WIDTH);
            let side = ((ui.available_width() - width) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    heading_row(app, ui, &t, &mut actions);
                    crumb_row(app, ui, &t, open_folder, &mut actions);
                    library_grid(app, ui, &t, open_folder, &mut actions);
                });
            });
        });
    });

    if let Some(done) = run_dialog(ui.ctx(), &t, dialog.as_ref()) {
        match done {
            DialogDone::Cancel => {
                ui.data_mut(|d| d.insert_temp(egui::Id::new(DIALOG_ID), None::<HomeDialog>));
            }
            DialogDone::Confirm(text) => {
                match dialog {
                    Some(HomeDialog::NewFolder) => {
                        app.library.create_folder(&text);
                    }
                    Some(HomeDialog::RenameFolder(id)) => app.library.rename_folder(id, &text),
                    None => {}
                }
                ui.data_mut(|d| d.insert_temp(egui::Id::new(DIALOG_ID), None::<HomeDialog>));
            }
        }
    }

    if let Some(dlg) = actions.open_dialog.take() {
        let text = match &dlg {
            HomeDialog::NewFolder => String::new(),
            HomeDialog::RenameFolder(id) => app.library.folder(*id).map(|f| f.name.clone()).unwrap_or_default(),
        };
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new(DIALOG_TEXT_ID), text);
            d.insert_temp(egui::Id::new(DIALOG_ID), Some(dlg));
        });
    }
    apply_actions(app, ui, actions);
}

fn apply_actions(app: &mut PdfCraftApp, ui: &mut egui::Ui, actions: CardActions) {
    if let Some(target) = actions.open_folder {
        ui.data_mut(|d| d.insert_temp(egui::Id::new(OPEN_FOLDER_ID), target));
    }
    if let Some((id, starred)) = actions.star {
        app.library.set_folder_starred(id, starred);
    }
    if let Some(id) = actions.delete_folder {
        app.library.delete_folder(id);
    }
    if let Some(id) = actions.rename_folder {
        // The text field is seeded when the dialog opens.
        let text = app.library.folder(id).map(|f| f.name.clone()).unwrap_or_default();
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new(DIALOG_TEXT_ID), text);
            d.insert_temp(egui::Id::new(DIALOG_ID), Some(HomeDialog::RenameFolder(id)));
        });
    }
    if let Some((path, folder)) = actions.move_doc {
        app.library.move_doc(&path, folder);
    }
    if let Some(path) = actions.remove_doc {
        app.library.remove_doc(&path);
    }
    if let Some(path) = actions.open_doc {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !std::path::Path::new(&path).exists() {
                app.library.set_missing(&path, true);
                app.notify_tr(tl!("The file was moved or deleted"));
                return;
            }
        }
        app.open_recent(&path);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(old) = actions.relocate_doc {
        let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_title(tl!("Locate file…"));
        app.ask_one(crate::pickers::Ask::File(dialog), None, move |app, path| {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            app.library.relocate(&old, &path.to_string_lossy(), &name);
        });
    }
}

enum DialogDone {
    Cancel,
    Confirm(String),
}

/// Draw the open dialog. The text lives in egui data so the frame that opens it seeds it.
fn run_dialog(ctx: &egui::Context, t: &Tokens, dialog: Option<&HomeDialog>) -> Option<DialogDone> {
    let dlg = dialog?;
    let id = match dlg {
        HomeDialog::NewFolder => "custom-home-dialog-new-folder",
        HomeDialog::RenameFolder(_) => "custom-home-dialog-rename-folder",
    };
    let title = match dlg {
        HomeDialog::NewFolder => tl!("New folder"),
        HomeDialog::RenameFolder(_) => tl!("Rename"),
    };
    let text_id = egui::Id::new(DIALOG_TEXT_ID);
    let mut done = None;
    let modal = widgets::show_modal(ctx, id, |ui| {
        ui.set_max_width(280.0);
        let title_label = ui.label(egui::RichText::new(title).font(theme::semibold(14.0)).color(t.text));
        ui.add_space(10.0);
        let mut text = ui.data_mut(|d| d.get_temp::<String>(text_id).unwrap_or_default());
        let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(240.0).hint_text(tl!("Name"))).labelled_by(title_label.id);
        // Focus the field while nothing else has focus: on open, and after clicking a button.
        if ui.memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        let mut submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            done = Some(DialogDone::Cancel);
        }
        ui.data_mut(|d| d.insert_temp(text_id, text.clone()));
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Create"), true).clicked() {
                submit = true;
            }
            ui.add_space(8.0);
            if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
                done = Some(DialogDone::Cancel);
            }
        });
        if submit {
            done = Some(DialogDone::Confirm(text));
        }
    });
    if modal.should_close() && done.is_none() {
        done = Some(DialogDone::Cancel);
    }
    done
}

fn heading_row(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, actions: &mut CardActions) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Documents")).font(theme::semibold(28.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill(ui, t, "folder-plus", tl!("New folder"), true) {
                actions.open_dialog = Some(HomeDialog::NewFolder);
            }
            ui.add_space(10.0);
            if pill(ui, t, "folder", tl!("Open file"), false) {
                app.open_dialog();
            }
        });
    });
}

fn crumb_row(app: &PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, open_folder: Option<u64>, actions: &mut CardActions) {
    ui.add_space(16.0);
    ui.horizontal(|ui| match open_folder.and_then(|id| app.library.folder(id)) {
        Some(folder) => {
            if ui.link(egui::RichText::new(tl!("All")).font(theme::medium(13.0))).clicked() {
                actions.open_folder = Some(None);
            }
            let (icon, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            icons::paint(ui, icon, "chevron-right", 14.0, t.text_faint);
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&folder.name).font(theme::medium(13.0)).color(t.text));
        }
        None => {
            let (icon, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
            icons::paint(ui, icon, "list", 15.0, t.text_muted);
            ui.add_space(7.0);
            ui.label(egui::RichText::new(tl!("All")).font(theme::medium(13.0)).color(t.text_muted));
        }
    });
}

fn library_grid(app: &PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, open_folder: Option<u64>, actions: &mut CardActions) {
    ui.add_space(24.0);
    let mut folder_items: Vec<&LibraryFolder> = Vec::new();
    let mut doc_items: Vec<&LibraryDoc> = Vec::new();
    if let Some(id) = open_folder {
        doc_items.extend(app.library.docs_in(Some(id)));
    } else {
        folder_items.extend(app.library.folders());
        // Starred folders first, then by creation order.
        folder_items.sort_by_key(|f| (!f.starred, f.id));
        doc_items.extend(app.library.docs_in(None));
    }
    if folder_items.is_empty() && doc_items.is_empty() {
        empty_note(
            ui,
            t,
            &if open_folder.is_some() {
                tl!("No PDFs in this folder yet").to_string()
            } else {
                tl!("Drop a PDF anywhere to open it, or create a folder to organize your documents.").to_string()
            },
        );
        return;
    }
    let cols = (((ui.available_width() + CARD_GAP) / (CARD_W + CARD_GAP)).floor() as usize).max(1);
    let items: Vec<GridItem<'_>> = folder_items.into_iter().map(GridItem::Folder).chain(doc_items.into_iter().map(GridItem::Doc)).collect();
    for row in items.chunks(cols) {
        ui.horizontal_top(|ui| {
            for item in row {
                match item {
                    GridItem::Folder(folder) => folder_card(app, ui, t, folder, actions),
                    GridItem::Doc(doc) => doc_card(app, ui, t, doc, actions),
                }
                ui.add_space(CARD_GAP);
            }
        });
        ui.add_space(CARD_GAP);
    }
}

enum GridItem<'a> {
    Folder(&'a LibraryFolder),
    Doc(&'a LibraryDoc),
}

fn folder_card(app: &PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, folder: &LibraryFolder, actions: &mut CardActions) {
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, FOLDER_CARD_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &folder.name));
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 10, false);
    // The folder floats up while hovered.
    let lift = press.hover * -2.0;
    let top = rect.top() + 10.0 + lift;
    let left = rect.center().x - 58.0;
    ui.painter().rect(
        Rect::from_min_size(egui::pos2(left + 6.0, top + 2.0), vec2(104.0, 30.0)),
        CornerRadius::same(6),
        folder_back(t),
        Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    ui.painter().rect(
        Rect::from_min_size(egui::pos2(left, top + 18.0), vec2(116.0, 56.0)),
        CornerRadius::same(10),
        folder_front(t),
        Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    // The star sits on the folder's top right; it appears when starred or hovered.
    let star_rect = Rect::from_center_size(egui::pos2(left + 98.0, top + 20.0), vec2(26.0, 26.0));
    let star = ui.interact(star_rect, ui.id().with(("lib-star", folder.id)), Sense::click());
    let star_label = if folder.starred {
        crate::i18n::fmt(tl!("Unstar {name}"), &[("name", &folder.name)])
    } else {
        crate::i18n::fmt(tl!("Star {name}"), &[("name", &folder.name)])
    };
    star.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &star_label));
    if folder.starred || star.hovered() {
        let tint = if folder.starred { STAR } else { t.icon };
        icons::paint(ui, star_rect.shrink(4.0), "star", 18.0, tint);
    }
    if star.clicked() {
        actions.star = Some((folder.id, !folder.starred));
    }
    // Name and "N files · newest" line.
    let count = app.library.folder_doc_count(folder.id);
    let count_text = crate::i18n::trn(crate::i18n::current(), count as u64, "{n} file", "{n} files");
    let meta = match app.library.folder_newest_added(folder.id) {
        Some(added) if added > 0 => format!("{count_text}  ·  {}", ago_of(added)),
        _ => count_text,
    };
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 118.0), &folder.name, theme::medium(13.5), t.text);
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 138.0), &meta, theme::regular(11.5), t.text_faint);
    if resp.clicked() && !star.clicked() {
        actions.open_folder = Some(Some(folder.id));
    }
    resp.context_menu(|ui| {
        if ui.button(tl!("Rename")).clicked() {
            actions.rename_folder = Some(folder.id);
            ui.close();
        }
        if ui.button(tl!("Delete")).clicked() {
            actions.delete_folder = Some(folder.id);
            ui.close();
        }
    });
}

fn doc_card(app: &PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, doc: &LibraryDoc, actions: &mut CardActions) {
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, DOC_CARD_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &doc.name));
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 10, false);
    let lift = press.hover * -2.0;
    let cover = Rect::from_min_size(egui::pos2(rect.center().x - 56.0, rect.top() + 2.0 + lift), vec2(112.0, 142.0));
    let thumbnail = if doc.missing { None } else { app.thumbnails.texture(ui.ctx(), &doc.path) };
    match thumbnail {
        Some(texture) => {
            // The page's own paper is the cover: fitted whole, with a soft shadow and a hairline.
            let size = texture.size_vec2();
            let fit = (cover.width() / size.x.max(1.0)).min(cover.height() / size.y.max(1.0));
            let page = Rect::from_center_size(cover.center(), size * fit);
            ui.painter().add(
                egui::Shadow { offset: [0, 3], blur: 8, spread: 0, color: t.page_shadow.gamma_multiply(0.5) }.as_shape(page, CornerRadius::same(3)),
            );
            ui.painter().image(texture.id(), page, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
            ui.painter().rect_stroke(page, CornerRadius::same(3), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        }
        None => {
            // Until the first page renders (or when it cannot): the fallback sheet and glyph.
            ui.painter().add(
                egui::Shadow { offset: [0, 3], blur: 10, spread: 0, color: t.page_shadow.gamma_multiply(0.45) }
                    .as_shape(cover, CornerRadius::same(10)),
            );
            ui.painter().rect(cover, CornerRadius::same(10), t.card, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            let icon_tint = if doc.missing { t.text_faint.gamma_multiply(0.45) } else { t.text_faint };
            icons::paint(ui, Rect::from_center_size(cover.center(), vec2(34.0, 34.0)), "file-text", 34.0, icon_tint);
        }
    }
    let meta = if doc.missing {
        (tl!("The file was moved or deleted")).to_string()
    } else if doc.pages > 0 {
        format!("{}  ·  {}", crate::i18n::trn(crate::i18n::current(), doc.pages as u64, "{n} page", "{n} pages"), ago_of(doc.added))
    } else {
        ago_of(doc.added)
    };
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 160.0), &doc.name, theme::medium(13.5), t.text);
    centered_elided(
        ui,
        t,
        egui::pos2(rect.center().x, rect.top() + 181.0),
        &meta,
        theme::regular(11.5),
        if doc.missing { t.text_muted } else { t.text_faint },
    );
    if resp.clicked() {
        actions.open_doc = Some(doc.path.clone());
    }
    resp.on_hover_text(&doc.path).context_menu(|ui| {
        ui.menu_button(tl!("Move to folder…"), |ui| {
            if ui.button(tl!("All")).clicked() {
                actions.move_doc = Some((doc.path.clone(), None));
                ui.close();
            }
            for folder in app.library.folders() {
                if ui.button(folder.name.as_str()).clicked() {
                    actions.move_doc = Some((doc.path.clone(), Some(folder.id)));
                    ui.close();
                }
            }
        });
        #[cfg(not(target_arch = "wasm32"))]
        if ui.button(tl!("Locate file…")).clicked() {
            actions.relocate_doc = Some(doc.path.clone());
            ui.close();
        }
        if ui.button(tl!("Remove from Documents")).clicked() {
            actions.remove_doc = Some(doc.path.clone());
            ui.close();
        }
    });
}

fn empty_note(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(text).font(theme::regular(13.0)).color(t.text_faint));
    });
    ui.add_space(48.0);
}

/// A soft or filled pill in the heading row.
fn pill(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str, primary: bool) -> bool {
    let font = theme::medium(13.0);
    let (rest, text_color) = if primary { (t.accent, Color32::WHITE) } else { (t.accent_soft, t.accent_text) };
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), text_color).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 48.0, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let press = theme::Press::track(ui, &resp);
    let fill = if primary {
        press.fill(rest, t.accent_text, t.accent.lerp_to_gamma(Color32::BLACK, 0.22))
    } else {
        press.fill(rest, t.selected, t.accent_soft.lerp_to_gamma(t.accent, 0.14))
    };
    ui.painter().rect(rect, CornerRadius::same(17), fill, Stroke::NONE, egui::StrokeKind::Inside);
    let body = rect.translate(press.offset());
    icons::paint(ui, Rect::from_min_size(body.min + vec2(13.0, 9.0), vec2(16.0, 16.0)), icon, 16.0, text_color);
    ui.painter().text(body.left_center() + vec2(35.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    theme::hand(resp).clicked()
}

/// One line of card text, centered, cut with "…" at the card's width.
fn centered_elided(ui: &egui::Ui, _t: &Tokens, center: egui::Pos2, text: &str, font: egui::FontId, color: Color32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: CARD_W - 14.0, max_rows: 1, break_anywhere: true, ..Default::default() };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let rect = Align2::CENTER_CENTER.anchor_size(center, galley.size());
    ui.painter().galley(rect.min, galley, color);
}

/// The folder colours follow the accent, so both themes keep the shelf's tint.
fn folder_front(t: &Tokens) -> Color32 {
    t.accent.lerp_to_gamma(Color32::WHITE, 0.55)
}

fn folder_back(t: &Tokens) -> Color32 {
    t.accent.lerp_to_gamma(Color32::WHITE, 0.42)
}

/// A Unix-seconds stamp as "yesterday", "3月10日" — empty for the unknown zero.
fn ago_of(unix: u64) -> String {
    if unix == 0 {
        return String::new();
    }
    crate::combine_ui::ago(UNIX_EPOCH + Duration::from_secs(unix))
}
