//! This fork's Home view: the document library — a GoodNotes-style shelf of user-created folders
//! and the PDFs the app has opened. References only: nothing here copies, moves or deletes a file.
//!
//! CUSTOM(kelongyan): whole-file customization; the upstream layout it replaces is forwarded
//! from `../home.rs`. See `Rule/fork-customization.md` (local only).

use std::time::{Duration, UNIX_EPOCH};

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, vec2};

use crate::custom::library::{FOLDER_PALETTE, LibraryDoc, LibraryFolder};
use crate::custom::tempo;
use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, icons, widgets};

/// Card metrics of the grid.
const CARD_W: f32 = 170.0;
const CARD_GAP: f32 = 24.0;
const DOC_CARD_H: f32 = 232.0;
/// The folder glyph on a card: one silhouette - a tab standing on a body - so the shelf reads as
/// folders at a glance instead of a pair of blocks stacked together.
const FOLDER_W: f32 = 148.0;
const FOLDER_H: f32 = 100.0;
const TAB_W: f32 = 40.0;
const TAB_H: f32 = 16.0;
/// The cover fills the card's width bar a 15-point margin, so the card reads as the page rather
/// than a frame around a small sheet: 170 - 2 * 15.
const COVER_W: f32 = 140.0;
const COVER_H: f32 = 176.0;
/// Home stays this wide and centered on a large window, so the page keeps one calm column.
const CONTENT_WIDTH: f32 = 1040.0;

/// A starred folder's star.
const STAR: Color32 = Color32::from_rgb(0xE2, 0xA8, 0x2E);

/// The open modal, if any. Stored in egui's per-context data, like other transient UI state.
const DIALOG_ID: &str = "custom-home-dialog";
const DIALOG_TEXT_ID: &str = "custom-home-dialog-text";
/// The new-folder sheet's colour choice, and its closing/pending state. The colour is chosen once,
/// at creation, and never changed afterwards.
const DIALOG_COLOR_ID: &str = "custom-home-dialog-colour";
const SHEET_CLOSING_ID: &str = "custom-home-sheet-closing";
const SHEET_PENDING_ID: &str = "custom-home-sheet-pending";
const SHEET_FOCUSED_ID: &str = "custom-home-sheet-focused";
/// The new-folder sheet: width, inner padding, resting top and how far it drops in from above.
const SHEET_W: f32 = 520.0;
const SHEET_PAD: f32 = 28.0;
const SHEET_REST_Y: f32 = 94.0;
const SHEET_TRAVEL: f32 = 420.0;
/// One colour swatch's box and the gap between them; eight fit a row.
const SWATCH: f32 = 26.0;
const SWATCH_GAP: f32 = 8.0;
/// Which folder is open; `None` (or a stale id) means the "All" top level.
const OPEN_FOLDER_ID: &str = "custom-home-open-folder";
/// Multi-select: whether cards select instead of open, and which documents are picked.
const SELECT_MODE_ID: &str = "custom-home-select-mode";
const SELECTED_ID: &str = "custom-home-selected";

/// What a Home document card hands to a folder card while it is dragged: the card's references
/// stay put, so a drop only files the document.
struct DocDrag {
    path: String,
}

fn select_mode(ui: &egui::Ui) -> bool {
    ui.data(|d| d.get_temp::<bool>(egui::Id::unique(SELECT_MODE_ID))).unwrap_or(false)
}

fn selected_paths(ui: &egui::Ui) -> Vec<String> {
    ui.data(|d| d.get_temp::<Vec<String>>(egui::Id::unique(SELECTED_ID))).unwrap_or_default()
}

fn set_selected(ui: &egui::Ui, paths: Vec<String>) {
    ui.data_mut(|d| d.insert_temp(egui::Id::unique(SELECTED_ID), paths));
}

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
    toggle_select: Option<String>,
    move_selected: Option<Option<u64>>,
    remove_selected: bool,
    open_dialog: Option<HomeDialog>,
}

pub(crate) fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let mut open_folder = ui.data_mut(|d| d.get_temp::<Option<u64>>(egui::Id::unique(OPEN_FOLDER_ID)).flatten());
    if open_folder.is_some_and(|id| app.library.folder(id).is_none()) {
        open_folder = None;
        ui.data_mut(|d| d.insert_temp(egui::Id::unique(OPEN_FOLDER_ID), open_folder));
    }
    let dialog = ui.data_mut(|d| d.get_temp::<Option<HomeDialog>>(egui::Id::unique(DIALOG_ID))).flatten();

    let mut actions = CardActions::default();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 30, bottom: 24 }).show(ui, |ui| {
            let width = ui.available_width().min(CONTENT_WIDTH);
            let side = ((ui.available_width() - width) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    heading_row(app, ui, &t, open_folder, &mut actions);
                    crumb_row(app, ui, &t, open_folder, &mut actions);
                    library_grid(app, ui, &t, open_folder, &mut actions);
                });
            });
        });
    });

    let done = match dialog.as_ref() {
        Some(HomeDialog::NewFolder) => run_new_folder_sheet(ui.ctx(), &t),
        Some(HomeDialog::RenameFolder(_)) => run_dialog(ui.ctx(), &t, dialog.as_ref()),
        None => None,
    };
    if let Some(done) = done {
        match done {
            DialogDone::Cancel => {}
            DialogDone::Rename(text) => {
                if let Some(HomeDialog::RenameFolder(id)) = dialog {
                    app.library.rename_folder(id, &text);
                }
            }
            DialogDone::Create { name, color } => {
                app.library.create_folder(&name, color);
            }
        }
        ui.data_mut(|d| d.insert_temp(egui::Id::unique(DIALOG_ID), None::<HomeDialog>));
    }

    if let Some(dlg) = actions.open_dialog.take() {
        let is_new = matches!(dlg, HomeDialog::NewFolder);
        let text = match &dlg {
            HomeDialog::NewFolder => String::new(),
            HomeDialog::RenameFolder(id) => app.library.folder(*id).map(|f| f.name.clone()).unwrap_or_default(),
        };
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::unique(DIALOG_TEXT_ID), text);
            d.insert_temp(egui::Id::unique(DIALOG_ID), Some(dlg));
            if is_new {
                d.insert_temp(egui::Id::unique(DIALOG_COLOR_ID), 0u8);
                d.insert_temp(egui::Id::unique(SHEET_CLOSING_ID), false);
                d.insert_temp(egui::Id::unique(SHEET_PENDING_ID), None::<DialogDone>);
                d.insert_temp(egui::Id::unique(SHEET_FOCUSED_ID), false);
            }
        });
    }
    apply_actions(app, ui, actions);
}

fn apply_actions(app: &mut PdfCraftApp, ui: &mut egui::Ui, actions: CardActions) {
    if let Some(target) = actions.open_folder {
        ui.data_mut(|d| d.insert_temp(egui::Id::unique(OPEN_FOLDER_ID), target));
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
            d.insert_temp(egui::Id::unique(DIALOG_TEXT_ID), text);
            d.insert_temp(egui::Id::unique(DIALOG_ID), Some(HomeDialog::RenameFolder(id)));
        });
    }
    if let Some((path, folder)) = actions.move_doc {
        app.library.move_doc(&path, folder);
    }
    if let Some(path) = actions.remove_doc {
        app.library.remove_doc(&path);
    }
    if let Some(path) = actions.toggle_select {
        let mut picked = selected_paths(ui);
        match picked.iter().position(|p| *p == path) {
            Some(i) => {
                picked.remove(i);
            }
            None => picked.push(path),
        }
        set_selected(ui, picked);
    }
    if let Some(folder) = actions.move_selected {
        let picked = selected_paths(ui);
        for path in &picked {
            app.library.move_doc(path, folder);
        }
        set_selected(ui, Vec::new());
    }
    if actions.remove_selected {
        let picked = selected_paths(ui);
        for path in &picked {
            app.library.remove_doc(path);
        }
        set_selected(ui, Vec::new());
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

#[derive(Clone)]
enum DialogDone {
    Cancel,
    Rename(String),
    Create { name: String, color: u8 },
}

/// Draw the rename dialog. The text lives in egui data so the frame that opens it seeds it.
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
    let text_id = egui::Id::unique(DIALOG_TEXT_ID);
    let mut done = None;
    let modal = widgets::show_modal(ctx, id, |ui| {
        ui.set_max_width(280.0);
        let title_label = ui.label(egui::RichText::new(title).font(theme::semibold(14.0)).color(t.text));
        ui.add_space(10.0);
        let mut text = ui.data_mut(|d| d.get_temp::<String>(text_id).unwrap_or_default());
        let resp = ui.add(widgets::line(&mut text).desired_width(240.0).hint_text(tl!("Name"))).labelled_by(title_label.id);
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
            done = Some(DialogDone::Rename(text));
        }
    });
    if modal.should_close() && done.is_none() {
        done = Some(DialogDone::Cancel);
    }
    done
}

/// The new-folder sheet: it drops in from above over a dimmed Home on the fork's `LARGE` rung, and
/// holds the folder preview, its name field and the colour grid. A folder keeps the colour chosen
/// here; the card has no colour control afterwards.
fn run_new_folder_sheet(ctx: &egui::Context, t: &Tokens) -> Option<DialogDone> {
    let closing = ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(SHEET_CLOSING_ID))).unwrap_or(false);
    let pending = ctx.data(|d| d.get_temp::<Option<DialogDone>>(egui::Id::unique(SHEET_PENDING_ID))).flatten();
    let progress = tempo::switch(ctx, egui::Id::unique("custom-home-sheet"), !closing, tempo::LARGE);
    if closing && progress <= 0.001 {
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::unique(SHEET_CLOSING_ID), false);
            d.insert_temp(egui::Id::unique(SHEET_PENDING_ID), None::<DialogDone>);
        });
        return pending.or(Some(DialogDone::Cancel));
    }

    let screen = ctx.content_rect();
    let veil = ((0.38 * progress).clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Area::new(egui::Id::unique("custom-home-sheet-veil")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(rect, egui::CornerRadius::same(0), Color32::from_black_alpha(veil));
        if resp.clicked() && !closing {
            begin_sheet_close(ctx, DialogDone::Cancel);
        }
    });

    let y = screen.top() + SHEET_REST_Y - (1.0 - progress) * SHEET_TRAVEL;
    let pos = egui::pos2(screen.center().x - SHEET_W * 0.5, y);
    let mut done = None;
    egui::Area::new(egui::Id::unique("custom-home-sheet")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        ui.set_width(SHEET_W);
        ui.set_opacity(progress.clamp(0.0, 1.0));
        let frame = egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(18))
            .inner_margin(egui::Margin::same(SHEET_PAD as i8))
            .shadow(ctx.global_style().visuals.window_shadow);
        frame.show(ui, |ui| {
            ui.set_width(SHEET_W - 2.0 * SHEET_PAD);
            done = sheet_body(ui, t, closing);
        });
    });
    if let Some(action) = done {
        begin_sheet_close(ctx, action);
    }
    None
}

fn begin_sheet_close(ctx: &egui::Context, action: DialogDone) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::unique(SHEET_CLOSING_ID), true);
        d.insert_temp(egui::Id::unique(SHEET_PENDING_ID), Some(action));
    });
}

/// The sheet's contents. Returns the action the user chose this frame.
fn sheet_body(ui: &mut egui::Ui, t: &Tokens, closing: bool) -> Option<DialogDone> {
    let text_id = egui::Id::unique(DIALOG_TEXT_ID);
    let color_id = egui::Id::unique(DIALOG_COLOR_ID);
    let mut text = ui.data(|d| d.get_temp::<String>(text_id)).unwrap_or_default();
    let mut color = ui.data(|d| d.get_temp::<u8>(color_id)).unwrap_or(0);
    let mut done = None;

    let heading = ui.vertical_centered(|ui| ui.label(egui::RichText::new(tl!("New folder")).font(theme::medium(12.5)).color(t.text_muted)));
    let heading_id = heading.inner.id;
    ui.add_space(6.0);
    let (preview, _) = ui.allocate_exact_size(vec2(ui.available_width(), 116.0), Sense::hover());
    paint_folder_preview(ui, t, color, preview);

    ui.add_space(12.0);
    let field = egui::Frame::new()
        .fill(t.pasteboard)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(18))
        .inner_margin(egui::Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut text)
                    .frame(egui::Frame::NONE)
                    .hint_text(tl!("Name"))
                    .font(theme::regular(14.0))
                    .desired_width(f32::INFINITY)
                    .align(egui::Align2::LEFT_CENTER),
            )
            .labelled_by(heading_id)
        })
        .inner;
    if !closing && !ui.data(|d| d.get_temp::<bool>(egui::Id::unique(SHEET_FOCUSED_ID)).unwrap_or(false)) {
        field.request_focus();
        ui.data_mut(|d| d.insert_temp(egui::Id::unique(SHEET_FOCUSED_ID), true));
    }
    ui.data_mut(|d| d.insert_temp(text_id, text.clone()));

    ui.add_space(18.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(tl!("Colour")).font(theme::medium(13.0)).color(t.text_muted));
    });
    ui.add_space(10.0);
    if let Some(picked) = colour_choices(ui, t, color)
        && !closing
    {
        color = picked;
    }
    ui.data_mut(|d| d.insert_temp(color_id, color));

    ui.add_space(18.0);
    if !closing {
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            done = Some(DialogDone::Cancel);
        }
        if enter && !text.trim().is_empty() {
            done = Some(DialogDone::Create { name: text.clone(), color });
        }
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let create = widgets::pill_button(ui, tl!("Create"), true).clicked();
        if create && !closing && !text.trim().is_empty() {
            done = Some(DialogDone::Create { name: text.clone(), color });
        }
        ui.add_space(8.0);
        if widgets::pill_button(ui, tl!("Cancel"), false).clicked() && !closing {
            done = Some(DialogDone::Cancel);
        }
    });
    done
}

/// The new-folder colour grid: three rows of eight, the current one ringed with a check.
fn colour_choices(ui: &mut egui::Ui, t: &Tokens, current: u8) -> Option<u8> {
    let width = 8.0 * SWATCH + 7.0 * SWATCH_GAP;
    let rows = FOLDER_PALETTE.len().div_ceil(8);
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) * 0.5).max(0.0));
        ui.vertical(|ui| {
            for row in 0..rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = SWATCH_GAP;
                    for col in 0..8 {
                        let i = row * 8 + col;
                        let Some(entry) = FOLDER_PALETTE.get(i) else { continue };
                        let index = i as u8;
                        let (rect, resp) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::click());
                        let name = tl!(entry.name);
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, index == current, name));
                        let center = rect.center();
                        let radius = 9.0 + if resp.hovered() { 0.5 } else { 0.0 };
                        ui.painter().circle_filled(center, radius, swatch_fill(t, index));
                        ui.painter().circle_stroke(center, radius, Stroke::new(1.0, t.border));
                        if resp.has_focus() {
                            ui.painter().circle_stroke(center, 12.5, Stroke::new(1.5, t.accent));
                        } else if index == current {
                            ui.painter().circle_stroke(center, 12.5, Stroke::new(1.5, t.text_muted));
                            let ink = if t.dark() { Color32::WHITE } else { t.text };
                            icons::paint(ui, Rect::from_center_size(center, vec2(12.0, 12.0)), "check", 12.0, ink);
                        } else if resp.hovered() {
                            ui.painter().circle_stroke(center, 12.5, Stroke::new(1.5, t.border_strong));
                        }
                        if resp.on_hover_text(name).clicked() {
                            picked = Some(index);
                        }
                    }
                });
                if row + 1 < rows {
                    ui.add_space(SWATCH_GAP);
                }
            }
        });
    });
    picked
}

/// The large folder on the sheet: the card's silhouette, tinted with the colour being chosen.
fn paint_folder_preview(ui: &egui::Ui, t: &Tokens, color: u8, rect: Rect) {
    const W: f32 = 132.0;
    const H: f32 = 92.0;
    const TAB_W_PREVIEW: f32 = 34.0;
    const TAB_H_PREVIEW: f32 = 14.0;
    let block_h = H + TAB_H_PREVIEW;
    let left = rect.center().x - W * 0.5;
    let body_top = rect.center().y - block_h * 0.5 + TAB_H_PREVIEW;
    let body = Rect::from_min_size(egui::pos2(left, body_top), vec2(W, H));
    let tab = Rect::from_min_size(egui::pos2(left, body_top - TAB_H_PREVIEW), vec2(TAB_W_PREVIEW, TAB_H_PREVIEW + 2.0));
    // The preview wears the colour at reading strength, unlike the pale tint the cards live on:
    // it is what the user is choosing, so the sheet shows it.
    let base = if color == 0 || usize::from(color) >= FOLDER_PALETTE.len() { t.accent } else { palette_base(color) };
    let (back, front) = if t.dark() {
        (base.lerp_to_gamma(DARK_TINT, 0.35), base.lerp_to_gamma(DARK_TINT, 0.35).lerp_to_gamma(Color32::WHITE, 0.10))
    } else {
        (base.lerp_to_gamma(Color32::WHITE, 0.20), base.lerp_to_gamma(Color32::WHITE, 0.45))
    };
    let edge = if t.dark() { t.border.gamma_multiply(0.9) } else { t.border };
    ui.painter().add(
        egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: t.page_shadow }
            .as_shape(body, egui::CornerRadius { nw: 0, ne: 16, sw: 16, se: 16 }),
    );
    ui.painter().rect(tab, egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, back, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    ui.painter().rect(body, egui::CornerRadius { nw: 0, ne: 16, sw: 16, se: 16 }, front, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
}

fn heading_row(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, open_folder: Option<u64>, actions: &mut CardActions) {
    let mut selecting = select_mode(ui);
    let selected = selected_paths(ui);
    ui.horizontal(|ui| {
        if selecting {
            // Selecting replaces the shelf's own actions with the ones that act on the picks, so
            // the row never carries both at once and stays one line tall.
            let title = if selected.is_empty() {
                tl!("Select documents").to_string()
            } else {
                crate::i18n::fmt(tl!("{n} selected"), &[("n", &selected.len().to_string())])
            };
            ui.label(egui::RichText::new(title).font(theme::semibold(24.0)).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, "x", 28.0, false, tl!("Done")).clicked() {
                    selecting = false;
                }
                ui.add_enabled_ui(!selected.is_empty(), |ui| {
                    if icons::button(ui, "trash-2", 28.0, false, tl!("Remove from Documents")).clicked() {
                        actions.remove_selected = true;
                    }
                    let move_btn = icons::button(ui, "move", 28.0, false, tl!("Move to folder…"));
                    egui::Popup::menu(&move_btn).show(|ui| {
                        if ui.button(tl!("All")).clicked() {
                            actions.move_selected = Some(None);
                            ui.close();
                        }
                        for folder in app.library.folders() {
                            if ui.button(folder.name.as_str()).clicked() {
                                actions.move_selected = Some(Some(folder.id));
                                ui.close();
                            }
                        }
                    });
                });
                if icons::button(ui, "check", 28.0, false, tl!("Select all")).clicked() {
                    let all = app.library.docs_in(open_folder).map(|d| d.path.clone()).collect();
                    set_selected(ui, all);
                }
            });
        } else {
            ui.label(egui::RichText::new(tl!("Documents")).font(theme::semibold(24.0)).color(t.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill(ui, t, "folder-plus", tl!("New folder"), true) {
                    actions.open_dialog = Some(HomeDialog::NewFolder);
                }
                ui.add_space(10.0);
                if pill(ui, t, "file-plus", tl!("Import…"), false) {
                    app.import_pdf_dialog(open_folder);
                }
                ui.add_space(2.0);
                if icons::button(ui, "list-checks", 28.0, false, tl!("Select multiple")).clicked() {
                    selecting = true;
                }
            });
        }
    });
    if selecting != select_mode(ui) {
        ui.data_mut(|d| d.insert_temp(egui::Id::unique(SELECT_MODE_ID), selecting));
        if !selecting {
            set_selected(ui, Vec::new());
        }
    }
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
                tl!("Imported PDFs stay here. A PDF you simply open shows in its tab, not in this list.").to_string()
            },
        );
        return;
    }
    let cols = (((ui.available_width() + CARD_GAP) / (CARD_W + CARD_GAP)).floor() as usize).max(1);
    let selecting = select_mode(ui);
    let selected = selected_paths(ui);
    let items: Vec<GridItem<'_>> = folder_items.into_iter().map(GridItem::Folder).chain(doc_items.into_iter().map(GridItem::Doc)).collect();
    for row in items.chunks(cols) {
        ui.horizontal_top(|ui| {
            for item in row {
                match item {
                    GridItem::Folder(folder) => folder_card(app, ui, t, folder, actions),
                    GridItem::Doc(doc) => doc_card(app, ui, t, doc, actions, selecting, selected.contains(&doc.path)),
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
    // CUSTOM(kelongyan): a folder is a card like a document - same height, same title baseline -
    // so a row that mixes the two stays on one grid instead of hanging 72 points short.
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, DOC_CARD_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &folder.name));
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 10, false);
    // A document dragged over the folder makes it rise and shows the accent outline; both ease
    // with the pointer on the micro rung, so crossing a folder never flashes it.
    let over = resp.dnd_hover_payload::<DocDrag>().is_some();
    let hot = tempo::switch(ui.ctx(), resp.id.with("drop-target"), over, tempo::MICRO);
    // The folder floats up while hovered, and a little more under a document about to land.
    let lift = press.hover * -2.0 - 2.0 * hot;
    // One silhouette on the cover's own baseline: a tab standing on a body, the tab a shade deeper
    // (the folder's back) and the body paler (its front panel), so the glyph keeps its depth without
    // a second colour. The body's top-left corner is square so the tab's left edge runs straight
    // into it; only the far corners are rounded. The block is centered on the document covers' own
    // slot, so a mixed row reads on one shelf instead of hanging off it.
    let block_h = TAB_H + FOLDER_H;
    let top = rect.top() + 2.0 + (COVER_H - block_h) * 0.5 + lift;
    let left = rect.center().x - FOLDER_W / 2.0;
    let body = Rect::from_min_size(egui::pos2(left, top + TAB_H), vec2(FOLDER_W, FOLDER_H));
    // A soft shadow lets the folder sit on the page rather than being pasted onto it; the hairline
    // keeps the outline crisp where the tint comes close to the pasteboard.
    let shadow = (0.55 + press.hover * 0.35 + hot * 0.25).min(1.0);
    ui.painter().add(
        egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: t.page_shadow.gamma_multiply(shadow) }
            .as_shape(body, CornerRadius { nw: 0, ne: 16, sw: 16, se: 16 }),
    );
    let (back, front) = folder_tints(t, folder.color);
    let edge = if t.dark() { t.border.gamma_multiply(0.9) } else { t.border };
    // Two pixels of overlap, hidden under the body, keep the join free of a seam.
    let tab = Rect::from_min_size(egui::pos2(left, body.top() - TAB_H), vec2(TAB_W, TAB_H + 2.0));
    ui.painter().rect(tab, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, back, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    ui.painter().rect(body, CornerRadius { nw: 0, ne: 16, sw: 16, se: 16 }, front, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    // A document dragged over the folder is dropped into it: the whole card is the target, and the
    // accent outline fades in while the pointer is over it.
    if hot > 0.001 {
        ui.painter().rect_stroke(
            body.expand(3.0),
            CornerRadius { nw: 4, ne: 18, sw: 18, se: 18 },
            Stroke::new(2.0, t.accent.gamma_multiply(hot)),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(drag) = resp.dnd_release_payload::<DocDrag>() {
        actions.move_doc = Some((drag.path.clone(), Some(folder.id)));
    }
    // The star sits on the folder's top right; it appears when starred or hovered.
    let star_rect = Rect::from_center_size(egui::pos2(body.right() - 20.0, body.top() + 20.0), vec2(26.0, 26.0));
    let star = ui.interact(star_rect, ui.scope_id().with(("lib-star", folder.id)), Sense::click());
    let star_label = if folder.starred {
        crate::i18n::fmt(tl!("Unstar {name}"), &[("name", &folder.name)])
    } else {
        crate::i18n::fmt(tl!("Star {name}"), &[("name", &folder.name)])
    };
    star.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &star_label));
    if folder.starred || star.hovered() {
        let tint = if folder.starred { STAR } else { t.icon };
        icons::paint(ui, star_rect.shrink(4.0), "star", 18.0, tint);
    }
    if star.clicked() {
        actions.star = Some((folder.id, !folder.starred));
    }
    // Name and "N files - newest" line, on the document cards' baseline.
    let count = app.library.folder_doc_count(folder.id);
    let count_text = crate::i18n::trn(crate::i18n::current(), count as u64, "{n} file", "{n} files");
    let meta = match app.library.folder_newest_added(folder.id) {
        Some(added) if added > 0 => format!("{count_text}  ·  {}", ago_of(added)),
        _ => count_text,
    };
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 196.0), &folder.name, theme::medium(15.0), t.text);
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 217.0), &meta, theme::regular(12.0), t.text_faint);
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

fn doc_card(app: &PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, doc: &LibraryDoc, actions: &mut CardActions, selecting: bool, selected: bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, DOC_CARD_H), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &doc.name));
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 10, selected);
    let lift = press.hover * -2.0;
    let cover = Rect::from_min_size(egui::pos2(rect.center().x - COVER_W / 2.0, rect.top() + 2.0 + lift), vec2(COVER_W, COVER_H));
    let thumbnail = if doc.missing { None } else { app.thumbnails.texture(ui.ctx(), &doc.path) };
    let thumb = thumbnail.as_ref().map(|t| (t.id(), t.size_vec2()));
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
    centered_elided(ui, t, egui::pos2(rect.center().x, rect.top() + 196.0), &doc.name, theme::medium(15.0), t.text);
    centered_elided(
        ui,
        t,
        egui::pos2(rect.center().x, rect.top() + 217.0),
        &meta,
        theme::regular(12.0),
        if doc.missing { t.text_muted } else { t.text_faint },
    );
    if selecting {
        // The pick ring sits on the sheet's top-left corner, the same place a file manager puts a
        // checkbox; the whole card toggles it, so the target is never a 18-point dot.
        let center = egui::pos2(cover.left() + 12.0, cover.top() + 12.0);
        ui.painter().circle_filled(center, 9.0, if selected { t.accent } else { t.card });
        ui.painter().circle_stroke(center, 9.0, Stroke::new(1.0, if selected { t.accent } else { t.border_strong }));
        if selected {
            icons::paint(ui, Rect::from_center_size(center, vec2(10.0, 10.0)), "check", 10.0, Color32::WHITE);
        }
    } else {
        // A card dragged onto a folder moves the document there; nothing is copied or deleted.
        resp.dnd_set_drag_payload(DocDrag { path: doc.path.clone() });
        if resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
    // Drag feedback: the page lifts out of the shelf as a ghost that follows the pointer on the
    // eased ladder (a short glide, never a snap), while the card left behind dims toward the
    // pasteboard. Both fade with the drag, so a release settles instead of blinking out.
    let dragging = !selecting && resp.dragged();
    let fade = tempo::switch(ui.ctx(), resp.id.with("drag-fade"), dragging, tempo::MICRO);
    let ghost_id = resp.id.with("drag-ghost");
    if dragging {
        let target = ui.ctx().pointer_interact_pos().unwrap_or(cover.center());
        let at = egui::pos2(
            tempo::track(ui.ctx(), resp.id.with("drag-x"), target.x, tempo::SLIDE),
            tempo::track(ui.ctx(), resp.id.with("drag-y"), target.y, tempo::SLIDE),
        );
        ui.data_mut(|d| d.insert_temp(ghost_id, at));
    }
    if fade > 0.001 {
        let at = ui.data(|d| d.get_temp::<egui::Pos2>(ghost_id)).unwrap_or(cover.center());
        ui.painter().rect_filled(rect, 12, t.pasteboard.gamma_multiply(0.55 * fade));
        paint_drag_ghost(ui.ctx(), t, at, cover.size(), thumb, fade);
    }
    if resp.clicked() {
        if selecting {
            actions.toggle_select = Some(doc.path.clone());
        } else {
            actions.open_doc = Some(doc.path.clone());
        }
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

/// The page a drag carries: the cover's own paper with a soft shadow, drawn on the tooltip layer
/// so no card paints over it. `fade` lifts it in and settles it back out on the micro rung; the
/// slight scale is part of that lift, so the page reads as picked up rather than pasted on.
fn paint_drag_ghost(ctx: &egui::Context, t: &Tokens, at: egui::Pos2, size: egui::Vec2, thumb: Option<(egui::TextureId, egui::Vec2)>, fade: f32) {
    let fade = fade.clamp(0.0, 1.0);
    let page = Rect::from_center_size(at, size * (1.0 + 0.04 * fade));
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("custom-home-drag-ghost")));
    painter.add(
        egui::Shadow { offset: [0, 8], blur: 22, spread: 0, color: t.page_shadow.gamma_multiply(0.8 * fade) }.as_shape(page, CornerRadius::same(6)),
    );
    match thumb {
        Some((id, texture)) => {
            let fit = (page.width() / texture.x.max(1.0)).min(page.height() / texture.y.max(1.0));
            let fitted = Rect::from_center_size(page.center(), texture * fit);
            let alpha = (255.0 * fade).round().clamp(0.0, 255.0) as u8;
            painter.image(id, fitted, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::from_white_alpha(alpha));
            painter.rect_stroke(fitted, CornerRadius::same(3), Stroke::new(1.0, t.border.gamma_multiply(fade)), egui::StrokeKind::Inside);
        }
        None => {
            painter.rect(
                page,
                CornerRadius::same(6),
                t.card.gamma_multiply(fade),
                Stroke::new(1.0, t.border.gamma_multiply(fade)),
                egui::StrokeKind::Inside,
            );
        }
    }
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
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
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

/// The near-black the dark theme's folder tints are mixed against, so a colour keeps its hue in
/// the shade instead of turning into a bright block.
const DARK_TINT: Color32 = Color32::from_rgb(0x24, 0x26, 0x2C);

/// The folder's two tints for the colour it wears: the tab is the back of the folder and a shade
/// deeper, the body the front panel a shade paler, so the glyph keeps its depth without a second
/// colour. Both stay close to the page, so a row of folders joins the shelf instead of shouting
/// over the white pages. Index 0 - and anything settings kept that the palette no longer has - is
/// the accent's own soft tint, exactly what folders wore before they had colours.
fn folder_tints(t: &Tokens, color: u8) -> (Color32, Color32) {
    if color == 0 || usize::from(color) >= FOLDER_PALETTE.len() {
        return if t.dark() {
            (t.accent_soft, t.accent_soft.lerp_to_gamma(Color32::WHITE, 0.09))
        } else {
            (t.accent_soft, t.accent_soft.lerp_to_gamma(Color32::WHITE, 0.16))
        };
    }
    let base = palette_base(color);
    if t.dark() {
        let back = base.lerp_to_gamma(DARK_TINT, 0.72);
        (back, back.lerp_to_gamma(Color32::WHITE, 0.09))
    } else {
        (base.lerp_to_gamma(Color32::WHITE, 0.80), base.lerp_to_gamma(Color32::WHITE, 0.90))
    }
}

/// A palette entry's hue; the accent stands in for an index the palette does not have.
fn palette_base(color: u8) -> Color32 {
    FOLDER_PALETTE.get(usize::from(color)).map_or(Color32::WHITE, |c| Color32::from_rgb(c.base[0], c.base[1], c.base[2]))
}

/// A palette circle's fill: the hue at reading weight in each theme - stronger than a folder tint,
/// because a swatch is a choice to look at, not a surface to live on.
fn swatch_fill(t: &Tokens, color: u8) -> Color32 {
    let base = if color == 0 { t.accent } else { palette_base(color) };
    if t.dark() { base.lerp_to_gamma(DARK_TINT, 0.42) } else { base.lerp_to_gamma(Color32::WHITE, 0.35) }
}

/// A Unix-seconds stamp as "yesterday", "3月10日" — empty for the unknown zero.
fn ago_of(unix: u64) -> String {
    if unix == 0 {
        return String::new();
    }
    crate::combine_ui::ago(UNIX_EPOCH + Duration::from_secs(unix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Home is a shelf of what the user imported: merely opening a PDF must not put it there, and
    /// a restart must not leave it behind either.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn only_an_imported_pdf_stays_in_documents() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let opened = dir.join("opened.pdf");
        let imported = dir.join("imported.pdf");
        let pdf = include_bytes!("../../tests/data/form.pdf");
        std::fs::write(&opened, pdf).expect("a scratch PDF");
        std::fs::write(&imported, pdf).expect("a scratch PDF");
        let opened = opened.to_string_lossy().into_owned();
        let imported = imported.to_string_lossy().into_owned();

        let mut app = PdfCraftApp::new();
        // File ▸ Open, a recent file, a double-click: the document opens, Home stays as it was.
        app.open_path(&opened);
        assert_eq!(app.views.len(), 1, "the document opened");
        assert!(app.library.docs().is_empty(), "opening a PDF does not put it on Home");
        // Home's Import button is the one way in.
        app.import_pending.push((imported.clone(), None));
        app.open_path(&imported);
        assert_eq!(app.library.docs().len(), 1, "importing keeps the PDF on Home");
        assert_eq!(app.library.docs()[0].path, imported);
        // And it is what settings keep: a restart shows the imported one, not the opened one.
        let saved = serde_json::to_value(&app.library).expect("library settings");
        let mut restarted = crate::custom::library::Library::default();
        restarted.restore(&saved);
        assert_eq!(restarted.docs().len(), 1);
        assert_eq!(restarted.docs()[0].path, imported);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A PDF imported while a folder is open lands in that folder, not at the top level.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_import_inside_a_folder_files_the_pdf_there() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-import-folder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let path = dir.join("filed.pdf");
        std::fs::write(&path, include_bytes!("../../tests/data/form.pdf")).expect("a scratch PDF");
        let path = path.to_string_lossy().into_owned();

        let mut app = PdfCraftApp::new();
        let folder = app.library.create_folder("Work", 0).expect("a folder");
        app.import_pending.push((path.clone(), Some(folder)));
        app.open_path(&path);
        assert_eq!(app.library.docs_in(Some(folder)).count(), 1, "the import is filed in the open folder");
        assert_eq!(app.library.docs_in(None).count(), 0, "and not at the top level");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A card's two lines stay one line each, and keep their gap, whatever language Home is in.
    ///
    /// Upstream checked this against its own tool cards; this fork draws Home, so the check belongs
    /// to the cards it actually shows. German is the language that tests it: its strings are long
    /// enough to want a second row, and a card whose lines collided would fail here rather than in
    /// a screenshot nobody took.
    #[test]
    fn translated_card_lines_stay_on_one_row_and_keep_their_gap() {
        const NAME: &str = "Jahreshauptversammlung-Und-Betriebspr\u{fc}fung-2026.pdf";
        for code in ["de", "en"] {
            let lang = crate::i18n::Lang::from_code(code).expect("a supported language");
            crate::i18n::set_current(lang);
            let ctx = egui::Context::default();
            ctx.set_fonts(theme::font_definitions());
            let mut app = PdfCraftApp::new();
            // A name long enough to want to wrap, and a count and date long enough to do it too.
            app.library.remember("/nowhere/card.pdf", NAME, 12345, 4096, 1_700_000_000);
            let output = ctx.run_ui(
                egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0))), ..Default::default() },
                |ui| show(&mut app, ui),
            );
            crate::i18n::set_current(crate::i18n::Lang::EN);

            // The card's name, which no catalogue translates, anchors the pair: the line under it is
            // the count and date the card shows in place of a status.
            let texts: Vec<(f32, String, usize, f32)> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((text.pos.y, text.galley.job.text.clone(), text.galley.rows.len(), text.galley.size().y)),
                    _ => None,
                })
                .collect();
            let title = texts.iter().find(|(_, text, _, _)| text.contains("Jahreshauptversammlung")).expect("the card shows its name");
            let meta = texts
                .iter()
                .find(|(y, text, _, _)| *y > title.0 && *y - title.0 < 40.0 && text.contains('\u{00B7}'))
                .expect("the card shows its count and date under the name");
            for &(y, _, rows, _) in [title, meta] {
                assert_eq!(rows, 1, "{code}: the card line at y={y} wrapped to {rows} rows");
            }
            // Both are anchored on their centre, so each takes half its height either side.
            let gap = (meta.0 - meta.3 / 2.0) - (title.0 + title.3 / 2.0);
            assert!(gap >= 4.0, "{code}: the card's lines crowd each other: gap {gap} between {:?} and {:?}", title.1, meta.1);
            output.drop_without_applying_deltas();
        }
    }
}
