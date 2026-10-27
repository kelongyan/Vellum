//! Home tab: this fork draws it with its own layout; the entry point stays here so the rest of
//! the shell keeps calling `home::show`.

use egui::{CornerRadius, Rect};

use crate::PdfCraftApp;
use crate::theme::{self, Tokens};

pub mod folders_ui;

/// A soft shadow that rises on hover and settles when the card is pressed.
///
/// Still the upstream helper: the All tools panel calls it (`panels/all_tools_ui.rs`), so a fork
/// takeover of Home must keep it.
pub(crate) fn card_lift(ui: &egui::Ui, t: &Tokens, rect: Rect, press: theme::Press) {
    let lift = press.hover * (1.0 - press.down);
    if lift > 0.04 {
        let shadow = egui::Shadow { offset: [0, 3], blur: (14.0 * lift) as u8, spread: 0, color: t.page_shadow.gamma_multiply(0.85 * lift) };
        ui.painter().add(shadow.as_shape(rect, CornerRadius::same(10)));
    }
}

/// Draw the Home page: the fork's document library (`custom/home.rs`).
pub fn show(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    // CUSTOM(kelongyan): the fork's Home layout lives in custom/home.rs (see Rule/fork-customization.md).
    crate::custom::home::show(app, ui);
}

// CUSTOM(kelongyan): upstream's `translated_card_description_leaves_room_for_the_action` test drew
// the upstream Home through `show`, which this fork replaces. The equivalent check lives in
// `custom/home.rs::translated_card_lines_stay_on_one_row_and_keep_their_gap`.
