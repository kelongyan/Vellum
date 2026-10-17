//! This fork's Home view: one centered column — the title row with the app icon and the open
//! button, recent files, tool chips, pinned folders, and one quiet privacy line.
//!
//! CUSTOM(kelongyan): whole-file customization; the upstream layout it replaces is forwarded
//! from `../home.rs`. See `Rule/fork-customization.md` (local only).

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, vec2};
use pdfcraft_engine::catalog;

use crate::folders_ui::{SHOWN, folder_name, location};
use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, icons, panels::human_size, widgets};

/// The tool chips on Home, in order.
const TOOLS: [&str; 8] = ["organize", "comment", "form", "edit", "protect", "combine", "scan", "export"];

/// Home stays this wide and centered on a large window, so the page keeps one calm column.
const CONTENT_WIDTH: f32 = 1040.0;

/// The app icon in the title row (the same artwork the window and taskbar use), drawn at 28 pt.
/// An SVG, like the other in-app brand art: only the SVG loader is installed (no PNG loader).
const APP_ICON: &[u8] = include_bytes!("../../../../assets/app-icon/pdfcraft-small.svg");

pub(crate) fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 30, bottom: 24 }).show(ui, |ui| {
            let width = ui.available_width().min(CONTENT_WIDTH);
            let side = ((ui.available_width() - width) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    title_row(app, ui, &t);
                    recent_section(app, ui, &t);
                    tools_section(app, ui, &t);
                    folders_section(app, ui, &t);
                    footnote(ui, &t);
                });
            });
        });
    });
}

fn title_row(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::from_bytes("bytes://pdfcraft-icon.svg", APP_ICON).fit_to_exact_size(vec2(28.0, 28.0)));
        ui.add_space(10.0);
        ui.label(egui::RichText::new("PdfCraft").font(theme::semibold(20.0)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if open_button(ui, t) {
                app.open_dialog();
            }
        });
    });
}

/// The one prominent action: a soft accent pill in the title row.
fn open_button(ui: &mut egui::Ui, t: &Tokens) -> bool {
    let label = tl!("Open file");
    let font = theme::medium(13.0);
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.accent_text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 48.0, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let press = theme::Press::track(ui, &resp);
    let fill = press.fill(t.accent_soft, t.selected, t.accent_soft.lerp_to_gamma(t.accent, 0.14));
    ui.painter().rect(rect, CornerRadius::same(17), fill, Stroke::NONE, egui::StrokeKind::Inside);
    let body = rect.translate(press.offset());
    icons::paint(ui, Rect::from_min_size(body.min + vec2(13.0, 9.0), vec2(16.0, 16.0)), "folder", 16.0, t.accent_text);
    ui.painter().text(body.left_center() + vec2(35.0, 0.0), Align2::LEFT_CENTER, label, font, t.accent_text);
    theme::hand(resp).clicked()
}

fn section_title(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::semibold(12.5)).color(t.text_muted));
}

fn recent_section(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(30.0);
    let mut clear = false;
    ui.horizontal(|ui| {
        section_title(ui, t, tl!("Recent"));
        if !app.recent.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = widgets::ghost_button(ui, "trash-2", tl!("Clear"));
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Clear Recent Files")));
                clear = resp.on_hover_text(tl!("Clear Recent Files")).clicked();
            });
        }
    });
    if app.recent.is_empty() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(tl!("Files you open in PdfCraft appear here. Drop a PDF anywhere to open it."))
                .font(theme::regular(12.5))
                .color(t.text_faint),
        );
    }
    let mut open = None;
    let mut remove = None;
    for r in &app.recent {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &r.name));
        let press = theme::Press::track(ui, &resp);
        // The row, not its response: the pointer stays "in" the row over its remove button.
        let hovered = ui.rect_contains_pointer(rect);
        if hovered && press.hover < 0.04 && press.down < 0.04 {
            // The remove button sits in the row, so the row response may not be hovered.
            ui.painter().rect_filled(rect, CornerRadius::same(8), t.hover);
        } else {
            press.wash(ui, rect, 8, false);
        }
        if hovered {
            // Remove just this file from the list (#430); the file itself is untouched.
            let x_rect = Rect::from_center_size(rect.right_center() - vec2(14.0, 0.0), vec2(28.0, 28.0));
            let x = ui.interact(x_rect, ui.id().with(("remove-recent", &r.path)), Sense::click());
            let label = crate::i18n::fmt(tl!("Remove {name} from Recent"), &[("name", &r.name)]);
            x.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if x.hovered() {
                ui.painter().rect_filled(x_rect, CornerRadius::same(6), t.pressed);
            }
            icons::paint(ui, x_rect.shrink(7.0), "x", 14.0, t.icon);
            if x.on_hover_text(tl!("Remove from Recent")).clicked() {
                remove = Some(r.path.clone());
            }
        }
        let body = rect.translate(press.offset());
        icons::paint(ui, Rect::from_min_size(body.min + vec2(10.0, 15.0), vec2(18.0, 18.0)), "file-text", 18.0, t.icon);
        // Room on the right for the remove button.
        let detail = ui.painter().text(
            body.right_center() - vec2(46.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{}  ·  {}", crate::i18n::trn(crate::i18n::current(), r.pages as u64, "{n} page", "{n} pages"), human_size(r.size)),
            theme::regular(12.0),
            t.text_faint,
        );
        // The name and path stop short of the page count; hovering shows the whole path.
        let width = detail.left() - 16.0 - (body.left() + 38.0);
        widgets::row_text(ui, body.min + vec2(38.0, 17.0), crate::bidi::visual(&r.name), theme::medium(13.5), t.text, width);
        widgets::row_text(ui, body.min + vec2(38.0, 34.0), crate::bidi::visual(&r.path), theme::regular(11.5), t.text_faint, width);
        if theme::hand(resp).on_hover_text(&r.path).clicked() {
            open = Some(r.path.clone());
        }
    }
    if let Some(p) = open {
        app.open_recent(&p);
    }
    if let Some(p) = remove {
        app.recent.retain(|r| r.path != p);
    }
    if clear {
        app.execute("file.clear_recent");
    }
}

fn tools_section(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(30.0);
    section_title(ui, t, tl!("Tools"));
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for id in TOOLS {
            let Some(g) = catalog::group(id) else { continue };
            if tool_chip(ui, t, g.icon, tl!(g.label)) {
                app.select_tool(g.id);
            }
        }
    });
}

/// A quiet pill: no border, a soft wash on hover, one monochrome icon.
fn tool_chip(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) -> bool {
    let font = theme::medium(13.0);
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 44.0, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let press = theme::Press::track(ui, &resp);
    let fill = press.fill(egui::Color32::TRANSPARENT, t.hover, t.pressed);
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, CornerRadius::same(17), fill);
    }
    let body = rect.translate(press.offset());
    icons::paint(ui, Rect::from_min_size(body.min + vec2(11.0, 9.0), vec2(16.0, 16.0)), icon, 16.0, t.icon);
    ui.painter().text(body.left_center() + vec2(33.0, 0.0), Align2::LEFT_CENTER, label, font, t.text);
    theme::hand(resp).clicked()
}

fn folders_section(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(30.0);
    app.refresh_pinned(ui.input(|i| i.time));
    let mut pin = false;
    ui.horizontal(|ui| {
        section_title(ui, t, tl!("Folders"));
        if !app.pinned.folders.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                pin = widgets::ghost_button(ui, "folder-plus", tl!("Pin a folder…")).clicked();
            });
        }
    });
    if app.pinned.folders.is_empty() {
        pin |= add_folder_box(ui, t);
    }
    let mut open = None;
    let mut unpin = None;
    let mut toggle = None;
    for folder in &app.pinned.folders {
        let name = folder_name(folder);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(crate::bidi::visual(&name)).font(theme::semibold(13.0))).on_hover_text(folder);
            ui.add_space(6.0);
            // Leave room for Unpin: a long location is cut short, never the button.
            let room = (ui.available_width() - 110.0).max(0.0);
            ui.scope(|ui| {
                ui.set_max_width(room);
                let place = egui::RichText::new(crate::bidi::visual(&location(folder))).font(theme::regular(11.5)).color(t.text_faint);
                ui.add(egui::Label::new(place).truncate()).on_hover_text(folder);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let resp = widgets::ghost_button(ui, "x", tl!("Unpin"));
                // Every folder has an Unpin button: say which one it is.
                let label = crate::i18n::fmt(tl!("Unpin {folder}"), &[("folder", &name)]);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
                if resp.clicked() {
                    unpin = Some(folder.clone());
                }
            });
        });
        let note = |ui: &mut egui::Ui, text: &str| {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(text).font(theme::regular(12.5)).color(t.text_faint));
        };
        let Some(listing) = app.pinned.listing(folder) else {
            note(ui, tl!("Looking for PDFs…"));
            continue;
        };
        if listing.unavailable {
            note(ui, tl!("This folder isn't available right now. It may have moved, or its drive or cloud sync is offline."));
            continue;
        }
        if listing.files.is_empty() {
            note(ui, tl!("No PDFs in this folder yet"));
            continue;
        }
        let expanded = app.pinned.expanded.contains(folder);
        let shown = if expanded { listing.files.len() } else { listing.files.len().min(SHOWN) };
        for f in listing.files.iter().take(shown) {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &f.name));
                let press = theme::Press::track(ui, &resp);
                press.wash(ui, rect, 8, false);
                let body = rect.translate(press.offset());
                icons::paint(ui, Rect::from_min_size(body.min + vec2(8.0, 13.0), vec2(16.0, 16.0)), "file-text", 16.0, t.icon);
                let size = human_size(usize::try_from(f.size).unwrap_or(usize::MAX));
                let when = f.modified.map(crate::combine_ui::ago).unwrap_or_default();
                let detail = if when.is_empty() { size } else { format!("{when}  ·  {size}") };
                let detail =
                    ui.painter().text(body.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, detail, theme::regular(12.0), t.text_faint);
                let width = detail.left() - 16.0 - (body.left() + 32.0);
                widgets::row_text(ui, body.min + vec2(32.0, 21.0), crate::bidi::visual(&f.name), theme::medium(13.0), t.text, width);
                if theme::hand(resp).on_hover_text(&f.path).clicked() {
                    open = Some(f.path.clone());
                }
            });
        }
        let more = listing.files.len().saturating_sub(shown);
        let capped = listing.total > listing.files.len();
        if more > 0 || (expanded && (listing.files.len() > SHOWN || capped)) {
            ui.horizontal(|ui| {
                ui.add_space(32.0);
                let label = if expanded { tl!("Show less").to_string() } else { crate::i18n::fmt(tl!("Show {n} more"), &[("n", &more.to_string())]) };
                if (more > 0 || listing.files.len() > SHOWN) && ui.link(label).clicked() {
                    toggle = Some(folder.clone());
                }
                if expanded && capped {
                    let n = listing.files.len().to_string();
                    let text = crate::i18n::fmt(tl!("The newest {n} of {total} PDFs"), &[("n", &n), ("total", &listing.total.to_string())]);
                    ui.label(egui::RichText::new(text).font(theme::regular(12.0)).color(t.text_faint));
                }
            });
        }
    }
    if pin {
        app.pin_folder_dialog();
    }
    if let Some(folder) = unpin {
        app.unpin_folder(&folder);
    }
    if let Some(folder) = toggle {
        if app.pinned.expanded.contains(&folder) {
            app.pinned.expanded.retain(|f| *f != folder);
        } else {
            app.pinned.expanded.push(folder);
        }
    }
    if let Some(path) = open {
        app.open_recent(&path);
    }
}

/// The empty-state entry point: a dashed box that reads as "add here".
fn add_folder_box(ui: &mut egui::Ui, t: &Tokens) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Pin a folder…")));
    let press = theme::Press::track(ui, &resp);
    let fill = press.fill(egui::Color32::TRANSPARENT, t.hover, t.pressed);
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    }
    let edge = t.border.lerp_to_gamma(t.text_muted, press.hover * 0.5);
    let dash = Stroke::new(1.0, edge);
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    for i in 0..4 {
        ui.painter().extend(egui::Shape::dashed_line(&[corners[i], corners[(i + 1) % 4]], dash, 5.0, 4.0));
    }
    let body = rect.translate(press.offset());
    let label = tl!("Pin a folder…");
    let font = theme::medium(12.5);
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text_faint).size().x);
    let left = body.center().x - (w + 23.0) * 0.5;
    icons::paint(ui, Rect::from_min_size(egui::pos2(left, body.center().y - 7.5), vec2(15.0, 15.0)), "plus", 15.0, t.text_faint);
    ui.painter().text(egui::pos2(left + 23.0, body.center().y), Align2::LEFT_CENTER, label, font, t.text_faint);
    theme::hand(resp).clicked()
}

fn footnote(ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(34.0);
    ui.label(
        egui::RichText::new(tl!("PdfCraft works offline. No telemetry, no account, and no cloud processing unless you add a provider."))
            .font(theme::regular(12.0))
            .color(t.text_faint),
    );
}
