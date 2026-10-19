//! The fork's motion ladder, and the tween that reads it.
//!
//! CUSTOM(kelongyan): the engine behind this fork's animation pass; the seams that call it are
//! marked in the upstream files. See `Rule/fork-customization.md` (local only).
//!
//! Upstream owns the switch. [`crate::motion`] zeroes `Style::animation_time` when Windows'
//! Animation effects is off, and every call here honours it: with animation effects off [`track`]
//! and [`switch`] answer with their target in the very pass they are asked, so "nothing in the
//! window eases" holds for the fork's motion exactly as it does for egui's own.
//!
//! Three rules keep this a system rather than a pile of effects:
//!
//! - A travel is a **timeline**, not a per-frame blend. The value at a moment depends on the
//!   clock, not on how many frames the machine happened to manage, so a dropped frame costs a jump
//!   along the same curve instead of a slow-motion stretch. A new target continues from wherever
//!   the value has got to, so an interrupted travel has no seam in it.
//! - Curves only **ease out**: movement starts at full speed and settles. Nothing winds up before
//!   it moves, and nothing overshoots and comes back.
//! - Travels are **asymmetric**, and the two directions mean two different things. A surface that
//!   comes and goes arrives slowly and leaves fast (`exit < enter`): appearing is calm, getting out
//!   of the way is quick. A state the pointer drives — hover, press — leaves slower than it
//!   arrives, because a pointer crossing the chrome would otherwise make the chrome flicker.

use std::collections::HashMap;

use egui::{Context, Id, emath};

/// The shape of a travel. Every curve in the ladder eases out; see the module docs.
pub type Curve = fn(f32) -> f32;

/// How a value reaches its target.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// Seconds travelling up.
    pub enter: f32,
    /// Seconds travelling down.
    pub exit: f32,
    pub curve: Curve,
}

/// A travel with no more than this much time left is over: the value snaps, so the last frame of
/// an animation is exactly its target and the window stops asking for frames on time.
const SNAP: f32 = 1e-4;

/// Two targets closer than this are the same target. A position a layout rounded differently in
/// its last bits is not a new target: aiming at it would restart a travel that had already
/// arrived, and ask for frames that move the value by less than a pixel. The ladder's values are
/// pixels and 0.0-1.0 mixes, where this is a thousandth of the former and a tenth of a percent of
/// the latter.
const SAME: f32 = 1e-3;

/// The sink of a press: the pointer has to feel it before it lifts. Slower to release than to
/// press, because the pointer is what ends it.
pub const TAP: Spec = Spec { enter: 0.06, exit: 0.10, curve: emath::easing::quadratic_out };

/// Hover, and anything else the pointer drives one pixel at a time.
pub const MICRO: Spec = Spec { enter: 0.09, exit: 0.15, curve: emath::easing::cubic_out };

/// Small surfaces: menus, tooltips, and the fade egui runs for every `Area` it opens.
///
/// `theme::apply` sets `Style::animation_time` to this rung's `enter`, so the surfaces egui fades
/// itself and the surfaces the fork travels by hand come and go on one clock. The two are not the
/// same shape — egui fades over `enter` in both directions with `quadratic_out`, while a fork
/// surface leaves over `exit` with `cubic_out` — so this is one clock, not one animation.
pub const SMALL: Spec = Spec { enter: 0.14, exit: 0.11, curve: emath::easing::cubic_out };

/// A move: the chrome sliding from one tab to the next, a panel taking its width. Both directions
/// take as long as each other — a position has no "coming" and "going", and a move that took
/// longer one way would read as the chrome leaning rather than moving.
pub const SLIDE: Spec = Spec { enter: 0.16, exit: 0.16, curve: emath::easing::cubic_out };

/// Cards that mean it: dialogs, the command palette, notices. `circular_out` decelerates hardest
/// of the ladder, which is what a surface that grew into place needs.
///
/// [`surface`] hands this rung to egui's own fade, which is what opens those cards: it measures its
/// fade with `Style::animation_time`, so a card gets its span without the fork having to work out
/// when the card appeared.
pub const MEDIUM: Spec = Spec { enter: 0.20, exit: 0.14, curve: emath::easing::circular_out };

/// Workspaces: a panel opening, the outline taking its room.
#[cfg_attr(not(test), expect(dead_code, reason = "P3 opens its panels against this rung"))]
pub const LARGE: Spec = Spec { enter: 0.26, exit: 0.18, curve: emath::easing::cubic_out };

/// Most travels kept, and how many are dropped when the cap is reached. A window this size reads a
/// few hundred a pass — every control's hover and press — so the cap guards against runaway ids (a
/// list that salts each row with its own generation), not against the ordinary page. Dropped
/// travels start again as if unseen, so the cap can cost an animation, never a wrong value.
///
/// The headroom matters as much as the cap: pruning back to it rather than to the cap means the
/// drop happens once every few hundred inserts instead of on every insert past the cap, which a
/// sort of the whole map per insert would not be cheap enough to afford at sixty frames a second.
const MAX_TRACKS: usize = 2048;
const KEEP_AFTER_PRUNE: usize = MAX_TRACKS * 3 / 4;

/// What a value is worth before anyone looked at it.
#[derive(Clone, Copy, Debug)]
enum Fresh {
    /// Already at the target. For values a widget has always had — a position, a width, a colour
    /// mix — where sliding in from nothing would be the widget moving, not the interface changing.
    At,
    /// Starting from `value`. For surfaces that come and go, where unseen is 0.0 and so opening
    /// one animates even the first time the window draws it.
    From(f32),
}

/// A travel: `from` at `started`, `to` `time` seconds later, along `curve`.
#[derive(Clone, Copy, Debug)]
struct Track {
    from: f32,
    to: f32,
    /// The egui time this travel began.
    started: f64,
    /// Seconds it takes: `enter` going up, `exit` going down. 0.0 when there is nowhere to go.
    time: f32,
    curve: Curve,
    /// The last pass that read it, for dropping the travels the window forgot about.
    seen: f64,
}

impl Track {
    /// A travel that is already where it is asked to be.
    fn settled(value: f32, now: f64) -> Self {
        Self { from: value, to: value, started: now, time: 0.0, curve: emath::easing::linear, seen: now }
    }

    /// The value now, and the seconds left before it stops moving. Pure in `now`: the same moment
    /// always reads the same value, which is what makes this a timeline rather than a blend.
    fn read(&self, now: f64) -> (f32, f32) {
        if self.time <= SNAP {
            return (self.to, 0.0);
        }
        let elapsed = (now - self.started).max(0.0) as f32;
        if elapsed >= self.time - SNAP {
            return (self.to, 0.0);
        }
        let eased = (self.curve)((elapsed / self.time).clamp(0.0, 1.0));
        // An easing function that answered with a non-number would end up in a shape's corner.
        if !eased.is_finite() {
            return (self.to, 0.0);
        }
        (self.from + (self.to - self.from) * eased, self.time - elapsed)
    }

    /// Aim at `target` from wherever this has got to. Up takes `enter`, down takes `exit`.
    fn aim(&mut self, target: f32, now: f64, spec: Spec) {
        let (from, _) = self.read(now);
        // Where it already is: nothing to travel. Without this a track mid-flight towards the value
        // it is now asked to hold would spend a whole `enter` asking for frames that repeat one.
        if from == target {
            *self = Self::settled(target, now);
            return;
        }
        let time = if target >= from { spec.enter } else { spec.exit }.max(0.0);
        *self = Self { from, to: target, started: now, time, curve: spec.curve, seen: now };
    }
}

/// The window's travels, keyed by the widget's own id.
#[derive(Clone, Default)]
struct Tracks {
    tracks: HashMap<u64, Track>,
}

impl Tracks {
    fn update(&mut self, id: Id, target: f32, spec: Spec, now: f64, fresh: Fresh) -> (f32, f32) {
        let key = id.value();
        if let Some(track) = self.tracks.get_mut(&key) {
            // A target a layout re-derived a hair differently is the same target: restarting the
            // travel for it would ask for a frame every pass to move the value by nothing.
            if (track.to - target).abs() > SAME {
                track.aim(target, now, spec);
            } else {
                track.seen = now;
            }
            return track.read(now);
        }
        let start = match fresh {
            Fresh::At => target,
            Fresh::From(value) => value,
        };
        let mut track = Track::settled(start, now);
        if start != target {
            track.aim(target, now, spec);
        }
        let read = track.read(now);
        self.tracks.insert(key, track);
        self.prune();
        read
    }

    /// Animation effects are off: keep the answer, drop the travel, so switching them back on does
    /// not replay what the user was told would not animate.
    fn settle(&mut self, id: Id, target: f32, now: f64) {
        self.tracks.insert(id.value(), Track::settled(target, now));
        self.prune();
    }

    fn prune(&mut self) {
        if self.tracks.len() <= MAX_TRACKS {
            return;
        }
        let over = self.tracks.len() - KEEP_AFTER_PRUNE;
        let mut oldest: Vec<(f64, u64)> = self.tracks.iter().map(|(key, track)| (track.seen, *key)).collect();
        // A total order, so the same window state drops the same travels on any machine.
        oldest.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, key) in oldest.into_iter().take(over) {
            self.tracks.remove(&key);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.tracks.len()
    }
}

fn tracks_id() -> Id {
    Id::new("pdfcraft-custom-tracks")
}

/// Whether the window animates this pass. The fork's surfaces ask this, as upstream's do
/// ([`crate::motion::animated`]); it is here so a surface needs one import for the whole ladder.
pub fn animated(ctx: &Context) -> bool {
    crate::motion::animated(ctx)
}

/// The value for `id`, travelling to `target` over `spec`. Call it every pass with the target you
/// want now: it answers with where the travel has got to and asks for the next pass while there is
/// travel left. The pass that first sees a new target still answers with the old value — the
/// travel begins there — so nothing ever jumps to cover a frame.
///
/// Use this for a value the widget always had, like a position, a width or a colour mix, where
/// unseen before means already there. A value that only moves takes [`SLIDE`], whose two
/// directions are the same length; [`SMALL`] and its neighbours are for surfaces that come and go.
/// For a surface coming and going, use [`switch`], which animates even the first time the window
/// draws it.
#[must_use = "the travel is the value; ask for it every pass"]
pub fn track(ctx: &Context, id: Id, target: f32, spec: Spec) -> f32 {
    read(ctx, id, target, spec, Fresh::At)
}

/// Draw a card that comes and goes - a dialog, the command palette, a notice - over `spec`.
///
/// egui fades the `Area` of every such card itself, and measures that fade with
/// `Style::animation_time`. This aims that fade at a rung for the one call that opens the card, so
/// the card gets [`MEDIUM`]'s span while menus and tooltips keep [`SMALL`]'s.
///
/// Deliberately not a hand-written travel: a card stops being drawn the frame it closes, so the
/// fork never gets to see the closing and could not ease it out. Deciding "this card is appearing"
/// by watching for a gap in the frames would replay the fade whenever a settled card happened to
/// be redrawn after a pause. egui already knows, from the area's own visibility, and gets it right.
///
/// The value is restored on the way out, so nothing else in the pass sees a borrowed clock. With
/// animation effects off there is no fade to aim, and `draw` runs on the style it was given.
pub fn surface<R>(ctx: &Context, spec: Spec, draw: impl FnOnce(&Context) -> R) -> R {
    if !animated(ctx) || !spec.enter.is_finite() || spec.enter <= 0.0 {
        return draw(ctx);
    }
    let previous = ctx.global_style().animation_time;
    ctx.global_style_mut(|style| style.animation_time = spec.enter);
    // Handed back when the borrow ends however it ends, including a card whose draw panicked on
    // its way to the last-resort guard: a clock left at a card's span would fade every menu in the
    // window at that span for the rest of the session.
    let _borrowed = Borrowed { ctx: ctx.clone(), previous };
    draw(ctx)
}

/// [`surface`]'s clock, on the way back.
struct Borrowed {
    ctx: Context,
    previous: f32,
}

impl Drop for Borrowed {
    fn drop(&mut self) {
        self.ctx.global_style_mut(|style| style.animation_time = self.previous);
    }
}

/// [`track`] between two states: 1.0 while `on`, 0.0 when not. Unseen before means not yet shown,
/// so the pass a dialog appears in it answers with 0.0 and travels in from there.
#[must_use = "the travel is the value; ask for it every pass"]
pub fn switch(ctx: &Context, id: Id, on: bool, spec: Spec) -> f32 {
    read(ctx, id, if on { 1.0 } else { 0.0 }, spec, Fresh::From(0.0))
}

fn read(ctx: &Context, id: Id, target: f32, spec: Spec, fresh: Fresh) -> f32 {
    let now = ctx.input(|i| i.time);
    // Nothing to place on a timeline: reach the target, or stay put for a target that isn't a
    // number. Either way no travel is remembered, so a bad value can't poison the shape that
    // reads it, and a clock that isn't a clock can't say how far anything has got.
    if !now.is_finite() || !target.is_finite() || !spec.enter.is_finite() || !spec.exit.is_finite() {
        return if target.is_finite() { target } else { 0.0 };
    }
    if !animated(ctx) {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Tracks>(tracks_id()).settle(id, target, now));
        return target;
    }
    let (value, left) = ctx.data_mut(|d| d.get_temp_mut_or_default::<Tracks>(tracks_id()).update(id, target, spec, now, fresh));
    if left > 0.0 {
        ctx.request_repaint();
    }
    value
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// One pass at `time`, drawing with `value`; how soon egui wants the next one.
    fn pass(ctx: &Context, time: f64, mut value: impl FnMut(&Context)) -> Duration {
        let mut out = ctx.run_ui(egui::RawInput { time: Some(time), ..Default::default() }, |ui| value(ui.ctx()));
        // There is no GPU here: the font atlas the first pass uploads is not a frame worth counting.
        out.textures_delta.clear();
        out.viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay)
    }

    /// [`switch`] through a pass: what it answered, and how soon the next pass is wanted.
    fn switch_at(ctx: &Context, time: f64, id: Id, on: bool, spec: Spec) -> (f32, Duration) {
        let mut answer = f32::NAN;
        let delay = pass(ctx, time, |ctx| answer = switch(ctx, id, on, spec));
        assert!(answer.is_finite(), "{id:?} answered {answer}");
        (answer, delay)
    }

    /// [`switch`] twice at the same moment. egui asks for two passes after any immediate request,
    /// so the second of these is the one that says whether anything is still moving.
    fn settled(ctx: &Context, time: f64, id: Id, on: bool, spec: Spec) -> (f32, Duration) {
        let _ = switch_at(ctx, time, id, on, spec);
        switch_at(ctx, time, id, on, spec)
    }

    /// [`track`] twice at the same moment, answering with the value.
    fn track_settled(ctx: &Context, time: f64, id: Id, target: f32, spec: Spec) -> f32 {
        let mut answer = f32::NAN;
        for _ in 0..2 {
            pass(ctx, time, |ctx| answer = track(ctx, id, target, spec));
        }
        assert!(answer.is_finite(), "{id:?} answered {answer}");
        answer
    }

    /// Past egui's own first passes, which ask for a frame for reasons of their own.
    fn warm_up(ctx: &Context) {
        let id = Id::new("warm-up");
        pass(ctx, 0.0, |ctx| {
            // Warming egui's own first passes, not reading the value.
            let _ = switch(ctx, id, false, MICRO);
        });
        settled(ctx, 0.02, id, false, MICRO);
    }

    #[test]
    fn an_idle_window_asks_for_no_frames() {
        let ctx = Context::default();
        // egui always draws one more pass after the first, so rest is two passes away.
        let first = pass(&ctx, 0.0, |ctx| {
            // The first pass is here to warm egui, not to read the value.
            let _ = switch(ctx, Id::new("idle"), false, MICRO);
        });
        assert_eq!(first, Duration::ZERO, "the bootstrap pass");
        let (value, delay) = settled(&ctx, 0.02, Id::new("idle"), false, MICRO);
        assert_eq!((value, delay), (0.0, Duration::MAX), "and nothing is wanted after that");
    }

    #[test]
    fn a_surface_arrives_over_its_enter_and_leaves_over_its_exit() {
        let ctx = Context::default();
        warm_up(&ctx);
        let id = Id::new("wash");
        // The pass it appears: none of it is there yet, and the next pass is wanted.
        let (first, delay) = switch_at(&ctx, 2.0, id, true, MICRO);
        assert_eq!(first, 0.0, "it starts out unseen");
        assert_eq!(delay, Duration::ZERO, "and the window wants the next frame: {delay:?}");
        // Halfway through `enter`, eased out, so most of the way already: cubic_out(0.5) is 0.875.
        let (half, _) = switch_at(&ctx, 2.0 + f64::from(MICRO.enter * 0.5), id, true, MICRO);
        assert!((half - 0.875).abs() < 1e-3, "cubic_out at half its time: {half}");
        let (whole, delay) = settled(&ctx, 2.0 + f64::from(MICRO.enter), id, true, MICRO);
        assert_eq!(whole, 1.0, "arrived");
        assert_eq!(delay, Duration::MAX, "and at rest it stops asking");
        // Leaving takes `exit`, the longer half: a pointer crossing the chrome must not flicker.
        let left_from = 2.0 + f64::from(MICRO.enter + MICRO.exit);
        let (held, _) = switch_at(&ctx, left_from, id, false, MICRO);
        assert_eq!(held, 1.0, "the pass that sees it go holds the whole wash: {held}");
        let (partly, _) = switch_at(&ctx, left_from + f64::from(MICRO.exit * 0.5), id, false, MICRO);
        assert!(partly > 0.0 && partly < 0.5, "a quarter of what it was at half its time: {partly}");
        let (gone, delay) = settled(&ctx, left_from + f64::from(MICRO.exit), id, false, MICRO);
        assert_eq!(gone, 0.0, "left");
        assert_eq!(delay, Duration::MAX, "and idle again");
    }

    #[test]
    fn the_answer_tells_the_time_not_the_frame_rate() {
        // The same moment of the same travel reads the same value whether the machine managed four
        // frames on the way there or two hundred: a slow frame jumps along the curve, it never
        // stretches the animation.
        let mut at_half_enter = Vec::new();
        for steps in [4_u32, 20, 200] {
            let ctx = Context::default();
            warm_up(&ctx);
            let id = Id::new("rate");
            switch_at(&ctx, 1.0, id, true, SMALL);
            let mut answer = f32::NAN;
            for step in 1..=steps {
                let time = 1.0 + f64::from(SMALL.enter) * (f64::from(step) / f64::from(steps));
                answer = switch_at(&ctx, time, id, true, SMALL).0;
                if step == steps / 2 {
                    break;
                }
            }
            assert!((answer - 0.875).abs() < 1e-3, "{steps} frames read {answer} at half its time");
            at_half_enter.push(answer);
        }
        for (i, value) in at_half_enter.iter().enumerate().skip(1) {
            assert!((value - at_half_enter[0]).abs() < 1e-6, "frame count {i} disagrees: {value} vs {}", at_half_enter[0]);
        }
    }

    #[test]
    fn a_new_target_continues_from_where_the_value_is() {
        let ctx = Context::default();
        warm_up(&ctx);
        let id = Id::new("interrupt");
        assert_eq!(switch_at(&ctx, 1.0, id, true, SMALL).0, 0.0, "the pass it appears");
        // Turn around a third of the way in: no jump, and it leaves from where it turned.
        let turned = 1.0 + f64::from(SMALL.enter / 3.0);
        let (mid, _) = switch_at(&ctx, turned, id, true, SMALL);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        let (back, _) = switch_at(&ctx, turned, id, false, SMALL);
        assert_eq!(back, mid, "turning around jumped from {mid} to {back}");
        let (partly, _) = switch_at(&ctx, turned + f64::from(SMALL.exit * 0.5), id, false, SMALL);
        assert!(partly > 0.0 && partly < mid, "{partly} is not between 0.0 and {mid}");
        // Reaching 0.0 takes `exit` from the turn, not from the top it never got to.
        assert_eq!(settled(&ctx, turned + f64::from(SMALL.exit), id, false, SMALL).0, 0.0, "arrived");
    }

    #[test]
    fn a_position_is_already_there_when_nobody_looked_at_it() {
        let ctx = Context::default();
        warm_up(&ctx);
        let id = Id::new("indicator");
        // `track` is for values the widget always had: the first read must not slide one in from 0.
        assert_eq!(track_settled(&ctx, 2.0, id, 120.0, SMALL), 120.0);
        // A change travels: the pass that first sees it holds the old value, so nothing jumps.
        assert_eq!(track_at(&ctx, 2.02, id, 220.0, SMALL), 120.0, "the travel begins here");
        let moved = track_at(&ctx, 2.04, id, 220.0, SMALL);
        assert!(moved > 120.0 && moved < 220.0, "{moved}");
        assert_eq!(track_settled(&ctx, 2.02 + f64::from(SMALL.enter), id, 220.0, SMALL), 220.0, "arrived");
        let turned = 2.02 + f64::from(SMALL.enter);
        assert_eq!(track_at(&ctx, turned, id, 120.0, SMALL), 220.0, "going back starts from 220");
        let back = track_at(&ctx, turned + f64::from(SMALL.exit * 0.5), id, 120.0, SMALL);
        assert!(back > 120.0 && back < 220.0, "and travels: {back}");
        assert_eq!(track_settled(&ctx, turned + f64::from(SMALL.exit), id, 120.0, SMALL), 120.0);
    }

    #[test]
    fn a_target_that_barely_moves_does_not_restart_the_travel() {
        let ctx = Context::default();
        warm_up(&ctx);
        let id = Id::new("jitter");
        assert_eq!(track_settled(&ctx, 2.0, id, 120.0, SMALL), 120.0);
        // A layout that rounds its last bits differently is not a new target: the value holds and
        // the window is not asked for another frame.
        let mut answer = f32::NAN;
        let delay = pass(&ctx, 2.02, |ctx| answer = track(ctx, id, 120.0005, SMALL));
        assert_eq!(answer, 120.0, "the value holds");
        assert_eq!(delay, Duration::MAX, "and nothing asks for frames: {delay:?}");
        // A target the ladder can see still travels, from wherever the value is.
        assert_eq!(track_at(&ctx, 2.04, id, 220.0, SMALL), 120.0, "the travel begins here");
        let moved = track_at(&ctx, 2.06, id, 220.0, SMALL);
        assert!(moved > 120.0 && moved < 220.0, "{moved}");
    }

    fn track_at(ctx: &Context, time: f64, id: Id, target: f32, spec: Spec) -> f32 {
        let mut answer = f32::NAN;
        pass(ctx, time, |ctx| answer = track(ctx, id, target, spec));
        assert!(answer.is_finite(), "{id:?} answered {answer}");
        answer
    }

    /// [`track`]'s internals through a pass at `time`, for the cases a plain call can't reach: a
    /// clock that isn't a number, a spec with no time, a curve that doesn't answer.
    fn read_at(ctx: &Context, id: Id, target: f32, spec: Spec, fresh: Fresh, time: f64) -> f32 {
        let mut answer = f32::NAN;
        pass(ctx, time, |ctx| answer = read(ctx, id, target, spec, fresh));
        assert!(answer.is_finite(), "{id:?} answered {answer}");
        answer
    }

    #[test]
    fn animation_effects_off_is_the_target_at_once() {
        let ctx = Context::default();
        warm_up(&ctx);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let id = Id::new("reduced");
        // Even the first sight: nothing travels, and nothing asks for a following frame.
        let (on, delay) = settled(&ctx, 0.2, id, true, MICRO);
        assert_eq!(on, 1.0, "the wash is simply there");
        assert_eq!(delay, Duration::MAX, "no travel to keep drawing");
        assert_eq!(settled(&ctx, 0.22, id, false, MICRO).0, 0.0);
        // Effects come back on later: what already happened does not replay.
        ctx.global_style_mut(|style| style.animation_time = 0.2);
        assert_eq!(settled(&ctx, 0.24, id, false, MICRO).0, 0.0);
        let (starts, delay) = switch_at(&ctx, 0.26, id, true, MICRO);
        assert_eq!(starts, 0.0, "a new travel starts from unseen");
        assert_eq!(delay, Duration::ZERO, "and asks for the frames it needs");
        let (half, _) = switch_at(&ctx, 0.26 + f64::from(MICRO.enter * 0.5), id, true, MICRO);
        assert!(half > 0.0 && half < 1.0, "{half}");
    }

    #[test]
    fn numbers_that_are_not_numbers_move_nothing() {
        let ctx = Context::default();
        warm_up(&ctx);
        let id = Id::new("bad");
        assert_eq!(track_at(&ctx, 1.0, id, 0.5, MICRO), 0.5, "settled as a baseline");
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(read_at(&ctx, id, bad, MICRO, Fresh::At, 1.0), 0.0, "{bad}");
            // A target that isn't a number leaves nothing behind: the good value comes straight back.
            assert_eq!(track_at(&ctx, 1.01, id, 0.5, MICRO), 0.5, "{bad} poisoned the travel");
        }
        // A clock that isn't a clock can't say how far anything has got. Its own window: egui keeps
        // the time it was given, and the next pass of a poisoned one would read oddly.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let broken_clock = Context::default();
            warm_up(&broken_clock);
            for (target, wanted) in [(0.5_f32, 0.5_f32), (f32::NAN, 0.0)] {
                assert_eq!(read_at(&broken_clock, id, target, MICRO, Fresh::From(0.0), bad), wanted, "{bad}");
            }
        }
        // A spec with no time is a move, not an animation.
        let none = Spec { enter: 0.0, exit: 0.0, curve: emath::easing::linear };
        assert_eq!(read_at(&ctx, Id::new("no-time"), 0.7, none, Fresh::At, 1.0), 0.7);
        assert_eq!(read_at(&ctx, Id::new("no-time"), 0.7, none, Fresh::From(0.0), 1.0), 0.7, "nothing to travel");
        // A curve that doesn't answer with a number loses its travel and arrives: a surface the
        // window can't animate is shown, never left out.
        let broken = Spec { enter: 0.1, exit: 0.1, curve: |_| f32::NAN };
        assert_eq!(read_at(&ctx, Id::new("broken"), 1.0, broken, Fresh::From(0.0), 1.0), 1.0);
        assert_eq!(read_at(&ctx, Id::new("broken"), 0.0, broken, Fresh::From(0.0), 1.05), 0.0);
    }

    #[test]
    fn the_surfaces_egui_fades_read_the_ladder() {
        // `theme::apply` is what hands egui's own fade the ladder's small-surface rung, so menus
        // and tooltips arrive over `SMALL.enter` rather than over egui's 0.2 s.
        let ctx = Context::default();
        assert_eq!(ctx.global_style().animation_time, 0.2, "what egui brings to us");
        crate::theme::apply(&ctx, crate::theme::ThemeKind::Light);
        assert_eq!(ctx.global_style().animation_time, SMALL.enter, "the fork's clock for the same surfaces");
        // A host that asked for no easing keeps it: this is where `motion` puts animation effects
        // off, and where the headless tests hold the picture still for determinism.
        crate::theme::apply(&ctx, crate::theme::ThemeKind::Dark);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        crate::theme::apply(&ctx, crate::theme::ThemeKind::Light);
        assert_eq!(ctx.global_style().animation_time, 0.0, "nothing travels, and nothing is restored");
    }

    #[test]
    fn a_surface_borrows_the_clock_its_fade_reads() {
        // egui measures an area's fade with `Style::animation_time`, so aiming a card at a rung
        // means lending that span for the one call that draws the card - and handing the clock
        // back, because everything else in the pass still reads the shared one.
        let ctx = Context::default();
        ctx.global_style_mut(|style| style.animation_time = SMALL.enter);
        let during = surface(&ctx, MEDIUM, |ctx| ctx.global_style().animation_time);
        assert_eq!(during, MEDIUM.enter, "the card is drawn on its own span");
        assert_eq!(ctx.global_style().animation_time, SMALL.enter, "and the clock goes back");
        // A card that fails to draw still gives the clock back.
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            surface(&ctx, LARGE, |_| panic!("the card failed to draw"));
        }));
        assert!(panicked.is_err(), "the panic travels on");
        assert_eq!(ctx.global_style().animation_time, SMALL.enter, "and the clock still goes back");
    }

    #[test]
    fn a_window_that_does_not_ease_borrows_nothing() {
        let ctx = Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let during = surface(&ctx, MEDIUM, |ctx| ctx.global_style().animation_time);
        assert_eq!(during, 0.0, "no span is lent when the host asked for no easing");
        // A rung with no time in it would otherwise stop egui's fade by setting 0.0 on a window
        // that does ease.
        ctx.global_style_mut(|style| style.animation_time = SMALL.enter);
        let none = Spec { enter: 0.0, exit: 0.0, curve: emath::easing::linear };
        let during = surface(&ctx, none, |ctx| ctx.global_style().animation_time);
        assert_eq!(during, SMALL.enter, "a rung with no span leaves the clock alone");
    }

    #[test]
    fn the_ladder_runs_up_and_the_two_directions_mean_two_things() {
        /// What a rung is for, which is what decides how its two directions relate.
        #[derive(Clone, Copy)]
        enum Direction {
            /// The pointer drives it and ends it, so leaving is the slower half.
            Pointer,
            /// A surface comes and goes: appearing is calm, getting out of the way is quick.
            Surface,
            /// A value that only moves, with no coming and going: both ways take as long.
            Move,
        }
        // Quickest to slowest: what the pointer drives, then what the user opens.
        let ladder = [
            (TAP, Direction::Pointer),
            (MICRO, Direction::Pointer),
            (SMALL, Direction::Surface),
            (SLIDE, Direction::Move),
            (MEDIUM, Direction::Surface),
            (LARGE, Direction::Surface),
        ];
        for (place, (spec, direction)) in ladder.iter().copied().enumerate() {
            assert!(spec.enter > SNAP && spec.exit > SNAP, "{spec:?}");
            assert!(spec.enter < 0.5, "{spec:?} is a wait, not an animation");
            assert_eq!((spec.curve)(0.0), 0.0, "{spec:?} does not start where it should");
            assert_eq!((spec.curve)(1.0), 1.0, "{spec:?} does not arrive where it should");
            let half = (spec.curve)(0.5);
            assert!((0.5..=1.0).contains(&half), "{spec:?} does not ease out: {half}");
            // Each rung is slower than the one before it, across the whole ladder.
            if place > 0 {
                assert!(ladder[place - 1].0.enter < spec.enter, "{spec:?} is not slower than {:?}", ladder[place - 1].0);
            }
            // What the pointer ends leaves slower than it arrived; a surface leaves faster; a
            // position has no directions to tell apart.
            match direction {
                Direction::Pointer => assert!(spec.exit > spec.enter, "{spec:?} leaves faster than it arrives"),
                Direction::Surface => assert!(spec.exit < spec.enter, "{spec:?} leaves slower than it arrives"),
                Direction::Move => assert_eq!(spec.exit, spec.enter, "{spec:?} is slower one way than the other"),
            }
        }
    }

    #[test]
    fn the_cap_holds_when_ids_run_away() {
        let ctx = Context::default();
        warm_up(&ctx);
        // Ten thousand ids over ten passes: what a list that re-salts every row would make of it.
        for batch in 0..10_u32 {
            let time = 1.0 + f64::from(batch) * 0.1;
            pass(&ctx, time, |ctx| {
                for i in 0..1000_u64 {
                    // Churning the map, not reading any of the values.
                    let _ = switch(ctx, Id::new(("many", batch, i)), true, MICRO);
                }
            });
        }
        let kept = ctx.data(|d| d.get_temp::<Tracks>(tracks_id()).map_or(0, |t| t.len()));
        assert!(kept <= MAX_TRACKS, "the cap holds: {kept}");
        assert!(kept >= KEEP_AFTER_PRUNE, "and the headroom was not thrown away: {kept}");
        // An id the window read in the last pass is still remembered and reads its target; one long
        // forgotten starts again from unseen: a lost animation, never a wrong value.
        assert_eq!(switch_at(&ctx, 2.0, Id::new(("many", 9, 0)), true, MICRO).0, 1.0, "still remembered");
        assert_eq!(switch_at(&ctx, 2.0, Id::new(("many", 0, 0)), true, MICRO).0, 0.0, "forgotten");
        // Reading them back grew the map by the one that was forgotten, and no more: forgetting is
        // bounded work, so a long session with a churning list stays around the cap.
        let after = ctx.data(|d| d.get_temp::<Tracks>(tracks_id()).map_or(0, |t| t.len()));
        assert!(after - kept <= 1, "the two reads added {after} from {kept}");
        assert!(after <= MAX_TRACKS, "and still under the cap: {after}");
    }
}
