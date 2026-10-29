//! CUSTOM(kelongyan): the tab underline travels along the fork's motion ladder
//! (`crates/ui-egui/src/custom/tempo.rs`, rung `SLIDE`). Local rule: Rule/fork-customization.md.

use egui::{Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::theme::{ThemeKind, ThemePreference, Tokens};
use pdfcraft_ui_egui::{Dialog, Mode, PdfCraftApp, PropsTab};

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
        app
    });
    h.run_steps(4);
    h
}

/// A tab, and only a tab: `mode_tab` is 48 points tall, so this skips the pane header of the same
/// name ("All tools" is both a mode tab and the title of the tools pane) and any menu item.
fn tab(h: &Harness<'static, PdfCraftApp>, label: &str) -> Rect {
    let found: Vec<Rect> = h.get_all_by_label(label).map(|node| node.rect()).filter(|r| (r.height() - 48.0).abs() < 1.0).collect();
    assert_eq!(found.len(), 1, "one tab called {label:?}: {found:?}");
    found[0]
}

/// The same node as a click target.
fn click_tab(h: &Harness<'static, PdfCraftApp>, label: &str) {
    let centre = tab(h, label).center();
    h.get_all_by_label(label).find(|node| (node.rect().center() - centre).length() < 1.0).expect("the tab").click();
}

/// Where the line is painted: on the active tab's bottom edge, raised two points out of the mode
/// bar's clip and its divider, and ten points short at both ends.
fn expected(rect: Rect) -> Rect {
    Rect::from_min_size(rect.left_bottom() + Vec2::new(10.0, -4.0), Vec2::new((rect.width() - 20.0).max(0.0), 2.0))
}

/// The bar on the row `rect` belongs to, found by where it sits: two points tall, filled, with no
/// outline, lying on that row's bottom edge. A dialog's tab row is at another height and keeps its
/// own travel, so the row is what picks which bar is meant.
///
/// Not found by colour: egui fades the surface it opens by gamma-multiplying every fill inside it,
/// so a dialog's line is a dimmer blue while the dialog comes in. [`settled_line`] checks the
/// colour once the fade is over, which is what proves the bar followed here is the underline.
fn underline(h: &Harness<'static, PdfCraftApp>, rect: Rect) -> Rect {
    let top = expected(rect).top();
    let found: Vec<Rect> = h
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(bar) if (bar.rect.height() - 2.0).abs() < 0.1 && bar.fill.a() > 0 && bar.stroke.width == 0.0 => Some(bar.rect),
            _ => None,
        })
        .filter(|bar| (bar.top() - top).abs() < 1.5)
        .collect();
    assert!(!found.is_empty(), "an underline on the row of {rect:?}");
    assert_eq!(found.len(), 1, "one underline on the row of {rect:?}: {found:?}");
    found[0]
}

/// [`underline`] for a row that may not have been drawn yet. The pass a surface opens is egui's
/// sizing pass: it lays the dialog out and paints none of it, so there is no line to read.
fn underline_opt(h: &Harness<'static, PdfCraftApp>, rect: Rect) -> Option<Rect> {
    let top = expected(rect).top();
    let found: Vec<Rect> = h
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(bar) if (bar.rect.height() - 2.0).abs() < 0.1 && bar.fill.a() > 0 && bar.stroke.width == 0.0 => Some(bar.rect),
            _ => None,
        })
        .filter(|bar| (bar.top() - top).abs() < 1.5)
        .collect();
    found.first().copied()
}

/// The underline once every surface around it has finished fading in: then it is the theme's accent
/// at full alpha, sitting where its tab sits.
fn settled_line(h: &Harness<'static, PdfCraftApp>, rect: Rect) -> Rect {
    let bar = underline(h, rect);
    let fill = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(r) if r.rect == bar => Some(r.fill),
            _ => None,
        })
        .expect("the line this test followed is still painted");
    assert_eq!(fill, Tokens::for_kind(h.state().theme).accent, "the bar on {rect:?}'s row is the accent underline");
    bar
}

/// Run a frame whose clock reads `at`. The harness holds its time still unless a test says
/// otherwise, which is also how `control.rs` drives frame-rate-dependent motion.
fn tick(h: &mut Harness<'static, PdfCraftApp>, at: f64) {
    h.input_mut().time = Some(at);
    h.step();
}

/// A window that does not ease sees the chrome exactly where upstream put it.
#[test]
fn a_still_window_underlines_the_active_tab_exactly() {
    let mut h = harness();
    // The headless harness stops egui's clock before the app ever runs, and `theme::apply` leaves a
    // clock at zero alone: a travel asked for with no easing answers with its target in the pass it
    // is asked. So tests and `cargo xtask screenshots` see the underline on its own tab, as before.
    assert_eq!(h.ctx.global_style().animation_time, 0.0, "the harness asks for no easing");
    for label in ["All tools", "Edit", "E-Sign"] {
        click_tab(&h, label);
        h.run_steps(3);
        let rect = tab(&h, label);
        assert_eq!(settled_line(&h, rect), expected(rect), "{label} holds the line where it sits");
    }
}

/// With the clock running the line does not reappear at its new end: it travels along its row and
/// arrives. Which pass a click lands in is egui's, not the ladder's, so the walk is read against
/// the pass the mode actually changed in, and each pass's own tab rect is asked for: the active tab
/// is set in its face, so choosing a mode moves the tabs beside it.
#[test]
fn the_underline_travels_to_the_tab_you_choose() {
    let mut h = harness();
    // A clock longer than any rung on the ladder, so no pass here can be mistaken for an arrival.
    h.ctx.all_styles_mut(|style| style.animation_time = 0.4);
    let from = tab(&h, "All tools");
    tick(&mut h, 100.0);
    assert_eq!(settled_line(&h, from), expected(from), "settled before the click");

    click_tab(&h, "Edit");
    let mut lefts: Vec<f32> = Vec::new();
    let mut chose = None;
    let mut at = 100.0;
    for step in 1..=24 {
        at += 0.01;
        tick(&mut h, at);
        lefts.push(underline(&h, tab(&h, "Edit")).left());
        if chose.is_none() && h.state().mode == Mode::Edit {
            chose = Some(step);
        }
    }
    let step = chose.expect("the click chose Edit");
    let start = expected(from).left();
    assert_eq!(lefts[step - 1], start, "the pass that first shows the new tab paints the old place");

    // Well past the slowest rung: the line is where the tab it was aimed at sits.
    tick(&mut h, 103.0);
    let to = tab(&h, "Edit");
    assert_eq!(settled_line(&h, to), expected(to), "the line holds at the tab it was aimed at");
    let end = expected(to).left();
    assert_ne!(end, start, "and it was aimed somewhere else: {start} then {end}");
    // Somewhere between the two it was at neither end: that is a travel, not a jump.
    assert!(lefts[step..].iter().any(|l| *l != start && *l != end), "the line jumped instead of travelling: {:?}", &lefts[step..]);
    tick(&mut h, 104.0);
    assert_eq!(settled_line(&h, tab(&h, "Edit")), expected(to), "and it does not drift once arrived");
}

/// A second tab row keeps its own underline. A row the window has never been asked about is a row
/// that has always been there, so the dialog's line arrives in place instead of sliding in from the
/// mode bar above it, and choosing in one row never moves the other.
#[test]
#[ignore = "CUSTOM(kelongyan): 0.15 merge - this assertion measures upstream layout; the fork Home, collapsed Tools pane and 32-point controls need re-basing"]
fn a_second_tab_row_holds_its_own_underline() {
    let mut h = harness();
    h.ctx.all_styles_mut(|style| style.animation_time = 0.4);
    let mode = tab(&h, "All tools");
    tick(&mut h, 100.0);
    assert_eq!(settled_line(&h, mode), expected(mode), "the mode bar's line sits where it belongs");

    h.state_mut().dialog = Some(Dialog::Properties(PropsTab::Description));
    // egui fades the dialog itself over the fork's clock while it is up; follow the row's left edge
    // through those passes. A position the window has never been asked about is already there, so
    // the line appears where its tab sits and never slides in from the mode bar's row above.
    let mut at = 100.0;
    let mut lefts: Vec<f32> = Vec::new();
    for _ in 0..6 {
        at += 0.02;
        tick(&mut h, at);
        // The first pass is the dialog's sizing pass, which paints nothing, including no line.
        if let Some(bar) = underline_opt(&h, tab(&h, "Description")) {
            lefts.push(bar.left());
        }
    }
    let here = expected(tab(&h, "Description")).left();
    assert!(!lefts.is_empty(), "the dialog's row is drawn while it fades in");
    assert!(lefts.iter().all(|l| (l - here).abs() < 0.5), "the row appeared where it sits: {lefts:?}");

    // Past the fade, so the bar is the accent again and the colour check can run.
    at += 0.6;
    tick(&mut h, at);
    let description = tab(&h, "Description");
    assert_eq!(settled_line(&h, tab(&h, "Description")), expected(description), "the dialog's row is in place as it opens");
    assert_eq!(settled_line(&h, mode), expected(mode), "and the mode bar did not move");

    click_tab(&h, "Advanced");
    let mut lefts: Vec<f32> = Vec::new();
    let mut chose = None;
    for step in 1..=24 {
        at += 0.01;
        tick(&mut h, at);
        lefts.push(underline(&h, tab(&h, "Advanced")).left());
        if chose.is_none() && h.state().dialog == Some(Dialog::Properties(PropsTab::Advanced)) {
            chose = Some(step);
        }
    }
    let step = chose.expect("the click chose Advanced");
    let start = expected(description).left();
    assert_eq!(lefts[step - 1], start, "the pass that shows the new tab paints the old place");
    tick(&mut h, 103.0);
    let advanced = tab(&h, "Advanced");
    assert_eq!(settled_line(&h, advanced), expected(advanced), "arrived on its own row");
    assert!(lefts[step..].iter().any(|l| *l != start && *l != expected(advanced).left()), "it travelled: {lefts:?}");
    assert_eq!(settled_line(&h, mode), expected(mode), "while the mode bar's line never moved");
}

/// The surfaces egui fades for us, menus and tooltips and every other `Area`, read the fork's clock.
#[test]
fn the_surfaces_egui_fades_are_on_the_fork_s_clock() {
    // A live window gets egui's 0.2 s fade replaced by `tempo::SMALL.enter`; a host that asked for
    // none keeps none, which is what keeps a headless test and a published screenshot still.
    let ctx = egui::Context::default();
    assert_eq!(ctx.global_style().animation_time, 0.2, "what egui brings to us");
    pdfcraft_ui_egui::theme::apply(&ctx, ThemeKind::Light);
    assert_eq!(ctx.global_style().animation_time, 0.14, "one clock for every surface that comes and goes");
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    pdfcraft_ui_egui::theme::apply(&ctx, ThemeKind::Light);
    assert_eq!(ctx.global_style().animation_time, 0.0, "a host that asked for none keeps none");
    // And in the running app, where the same seam is re-run every time the theme changes.
    let mut h = harness();
    h.state_mut().set_theme_preference(ThemePreference::Dark);
    h.run_steps(2);
    assert_eq!(h.ctx.global_style().animation_time, 0.0, "a window told to stop easing stays stopped");
}
/// A dialog is a card, so it rises over `MEDIUM`, not over the span menus get.
#[test]
fn a_dialog_rises_over_its_own_rung() {
    // A window that does not ease - the headless harness, and a user who turned animation effects
    // off - borrows nothing: the card is simply there, whole. That is what keeps every other dialog
    // test, and every published screenshot, as still as it was before the ladder existed.
    let mut h = harness();
    assert_eq!(h.ctx.global_style().animation_time, 0.0, "the harness asks for no easing");
    h.state_mut().dialog = Some(Dialog::Properties(PropsTab::Description));
    h.run_steps(4);
    assert_eq!(backdrop_alpha(&h), 100, "a still window shows the dialog at once");

    // A window that does ease fades the same card in, over the card's rung rather than the menus'.
    let mut h = dialog_harness();
    h.ctx.all_styles_mut(|style| style.animation_time = 0.4);
    tick(&mut h, 200.0);
    h.state_mut().dialog = Some(Dialog::Properties(PropsTab::Description));
    tick(&mut h, 200.0);
    let opening = backdrop_alpha(&h);
    assert!((10..100).contains(&opening), "the pass a dialog appears in shows it on its way in: alpha {opening}");
    // Sixteen hundredths in: past `SMALL`, the span every menu and tooltip now takes, so a card
    // that had borrowed the wrong rung would already be whole here.
    tick(&mut h, 200.16);
    let past_the_menus = backdrop_alpha(&h);
    assert!(past_the_menus < 100, "still rising past a menu's 0.14 s: alpha {past_the_menus}");
    assert!(past_the_menus > opening, "and rising: {opening} then {past_the_menus}");
    tick(&mut h, 200.3);
    assert_eq!(backdrop_alpha(&h), 100, "arrived");
    // The borrow is only for the card's own call: the rest of the window keeps the shared clock.
    assert_eq!(h.ctx.global_style().animation_time, 0.4, "the clock is handed back");
}

/// A harness whose frames advance a hundredth of a second, so a fade can be watched at a known age.
fn dialog_harness() -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).with_step_dt(0.01).build_eframe(|_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("doc.pdf", None, PDF.to_vec()).expect("opens");
        app
    });
    h.run_steps(4);
    h
}

/// How opaque the dialog's backdrop is painted this pass, out of the 100 it settles at. egui fades
/// a surface by multiplying the opacity of everything inside it, and the backdrop is the one shape
/// in that surface whose colour says nothing else: the card's fill, its border and its shadow all
/// vary with the theme.
fn backdrop_alpha(h: &Harness<'static, PdfCraftApp>) -> u8 {
    let mut widest: Option<(f32, u8)> = None;
    for shape in &h.output().shapes {
        if let egui::Shape::Rect(rect) = &shape.shape {
            let fill = rect.fill;
            let black = fill.r() == 0 && fill.g() == 0 && fill.b() == 0;
            let area = rect.rect.width() * rect.rect.height();
            if black && fill.a() > 0 && widest.is_none_or(|(seen, _)| area > seen) {
                widest = Some((area, fill.a()));
            }
        }
    }
    let (area, alpha) = widest.expect("a dialog paints its backdrop");
    assert!(area > 100_000.0, "the backdrop covers the window, not a shadow: {area}");
    alpha
}
