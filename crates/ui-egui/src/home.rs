//! Home tab: this fork draws it with its own layout; the entry point stays here so the rest of
//! the shell keeps calling `home::show`.

use crate::PdfCraftApp;

pub fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    // CUSTOM(kelongyan): the fork's Home layout lives in custom/home.rs (see Rule/fork-customization.md).
    crate::custom::home::show(app, ui);
}
