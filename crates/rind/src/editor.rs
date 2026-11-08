// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rind's editor: the control logic (platform independent: drags with the cursor held in place, taps, the wheel, reset
//! gestures) and, with the `embed` feature, the window glue that runs it in Honeydew's own window
//! (`os_plugkit::embed`).
//!
//! The glue follows the template of Lemon Studio's editors (`lemon_small_fx::editor`): an input loop over the window's
//! inputs that ends when none is waiting, where the editor parks (`os_plugkit::park!`) and Honeydew runs it on when it
//! brings more. Edits go through `os_plugkit::vals::Vals`, so they show and sound at once ([`crate::params::RindParams::live`])
//! and reach the host as gestures.
//!
//! Reset gestures: Honeydew's card turns a double tap on a control that holds the cursor (a knob or fader) into a middle
//! click before the drag goes on, so the middle button and Alt-click reset to the default here, and a press never
//! looks at `dbl`.
// Without `embed` there is no window to run the logic: only the tests do.
#![cfg_attr(not(feature = "embed"), allow(dead_code))]

use std::any::Any;
use std::sync::Arc;
#[cfg(feature = "embed")]
use std::sync::atomic::{AtomicU32, Ordering};

use nih_plug::prelude::*;
use os_plugkit::vals::Vals;
use os_ui::Canvas;

use crate::ids::{Kind as Range, SPECS};
use crate::params::RindParams;
use crate::view::{self, Hit, Kind, Ui, Values};

/// The modifier keys that matter here, as held at an input: Ctrl (fine steps) and Alt (reset).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Left,
    Right,
    Middle,
}

/// What the window must do after an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Repaint,
    /// Hold the cursor: movement comes as deltas until the press ends.
    Lock,
}

/// Which way a drag runs: up raises a knob, number or fader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Up,
}

/// A drag: `v` is the unrounded normalised value it has reached, over `px` for the whole range.
#[derive(Clone, Copy, Debug)]
struct Drag {
    param: usize,
    v: f32,
    px: f32,
    axis: Axis,
}

/// The values of a [`Vals`], by table index.
struct Shown<'a>(&'a Vals);

impl Values for Shown<'_> {
    fn norm(&self, p: usize) -> f32 {
        self.0.norm(SPECS[p].id)
    }
    fn plain(&self, p: usize) -> f32 {
        self.0.plain(SPECS[p].id)
    }
    fn text(&self, p: usize) -> String {
        self.0.text(SPECS[p].id)
    }
}

/// The editor's logic.
pub struct Logic {
    pub vals: Vals,
    ui: Ui,
    hits: Vec<Hit>,
    drag: Option<Drag>,
    pressed: Option<usize>,
    last: Option<usize>,
}

/// A fine drag or wheel step (Ctrl) is this share of a normal one.
const FINE: f32 = 0.25;
/// A wheel notch moves a knob or fader this much of its range.
const WHEEL_STEP: f32 = 0.02;

impl Logic {
    pub fn new(vals: Vals) -> Self {
        let ui = Ui::default();
        Logic {
            vals,
            hits: view::layout(&ui),
            ui,
            drag: None,
            pressed: None,
            last: None,
        }
    }

    /// Paints the editor and keeps its hit regions.
    pub fn paint(&mut self, c: &mut Canvas) {
        let hits = view::paint(c, &Shown(&self.vals), &self.ui, self.pressed, self.last);
        self.hits = hits;
    }

    /// The regions of the screen as it is now (after a tab, sub tab or list changed).
    fn relayout(&mut self) {
        self.hits = view::layout(&self.ui);
    }

    /// The control at (`x`, `y`); a touch that just misses a small control still reaches the nearest one.
    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        if let Some(h) = self.hits.iter().rev().find(|h| h.r.contains(x, y)) {
            return Some(*h);
        }
        const SLOP: f32 = 5.0;
        let near = |h: &&Hit| {
            matches!(
                h.kind,
                Kind::Stepper | Kind::Cycle | Kind::Toggle | Kind::Nav(_)
            ) && h.r.inset(-SLOP).contains(x, y)
        };
        let dist = |h: &Hit| (h.r.cx() - x).powi(2) + (h.r.cy() - y).powi(2);
        self.hits
            .iter()
            .filter(near)
            .min_by(|a, b| dist(a).total_cmp(&dist(b)))
            .copied()
    }

    fn reset(&mut self, p: usize) {
        let id = SPECS[p].id;
        let default = self.vals.default_norm(id);
        self.vals.edit(id, default);
    }

    fn begin_drag(&mut self, p: usize, px: f32, axis: Axis, at: Option<f32>) -> Vec<Act> {
        let id = SPECS[p].id;
        self.vals.begin(id);
        if let Some(v) = at {
            self.vals.set(id, v);
        }
        self.drag = Some(Drag {
            param: p,
            v: at.unwrap_or_else(|| self.vals.norm(id)),
            px,
            axis,
        });
        self.pressed = Some(p);
        vec![Act::Repaint, Act::Lock]
    }

    /// The next (`step` 1) or previous (-1) whole value of `p`, wrapping when `wrap`.
    fn step(&mut self, p: usize, step: i32, wrap: bool) {
        let Range::Int(lo, hi) = SPECS[p].kind else {
            return;
        };
        let now = self.vals.plain(SPECS[p].id).round() as i32;
        let mut to = now + step;
        if to > hi {
            to = if wrap { lo } else { hi };
        }
        if to < lo {
            to = if wrap { hi } else { lo };
        }
        self.vals.edit_plain(SPECS[p].id, to as f32);
    }

    /// A press at (`x`, `y`) in logical px.
    pub fn down(&mut self, x: f32, y: f32, btn: Btn, m: Mods) -> Vec<Act> {
        let Some(h) = self.hit(x, y) else {
            return Vec::new();
        };
        let p = h.param;
        match h.kind {
            Kind::Nav(n) => {
                if btn == Btn::Left {
                    self.ui.apply(n);
                    self.relayout();
                    return vec![Act::Repaint];
                }
                return Vec::new();
            }
            // A name in the open list: pick it and close the list (only a left press picks).
            Kind::Item(i) => {
                if btn == Btn::Left {
                    let (_, lo) = view::choices(p);
                    self.claim(p);
                    let to = (lo + i as i32) as f32;
                    self.vals.edit_plain(SPECS[p].id, to);
                    // The second row of a group follows its source.
                    if let Some(q) = view::partner(p) {
                        self.vals.edit_plain(SPECS[q].id, to);
                    }
                    self.last = Some(p);
                }
                self.ui.popup = None;
                self.relayout();
                return vec![Act::Repaint];
            }
            // Anywhere else: the list closes and the press goes no further.
            Kind::Dismiss => {
                self.ui.popup = None;
                self.relayout();
                return vec![Act::Repaint];
            }
            _ => {}
        }
        self.last = Some(p);
        match (btn, m.alt) {
            (Btn::Middle, _) | (Btn::Left, true) => {
                self.claim(p);
                self.reset(p)
            }
            (Btn::Right, _) => {
                if h.kind == Kind::Cycle {
                    self.claim(p);
                    self.step(p, -1, true);
                } else {
                    return vec![Act::Repaint];
                }
            }
            (Btn::Left, false) => {
                self.claim(p);
                match h.kind {
                    Kind::Cycle if view::dropdown(p) => {
                        self.ui.popup = Some(p);
                        self.relayout();
                    }
                    Kind::Cycle | Kind::Toggle => self.step(p, 1, true),
                    Kind::Stepper => {
                        let at = (x - h.r.x) / h.r.w;
                        if at < view::STEP_END {
                            self.step(p, -1, false);
                        } else if at > 1.0 - view::STEP_END {
                            self.step(p, 1, false);
                        } else if let Range::Int(lo, hi) = SPECS[p].kind {
                            // The middle drags: a whole value per 14 px.
                            let px = (14.0 * (hi - lo) as f32).max(40.0);
                            return self.begin_drag(p, px, Axis::Up, None);
                        }
                    }
                    Kind::Knob => return self.begin_drag(p, view::KNOB_PX, Axis::Up, None),
                    Kind::Num => return self.begin_drag(p, view::NUM_PX, Axis::Up, None),
                    Kind::Fader => {
                        let at = view::fader_norm(h.r, y);
                        let px = h.r.h - 2.0 * view::FADER_PAD;
                        return self.begin_drag(p, px, Axis::Up, Some(at));
                    }
                    Kind::Nav(_) | Kind::Item(_) | Kind::Dismiss => {}
                }
            }
        }
        vec![Act::Repaint]
    }

    /// A row whose source the screen decides takes it before it is touched.
    fn claim(&mut self, p: usize) {
        if let Some((src, want)) = view::row_source(&Shown(&self.vals), p)
            && self.vals.plain(SPECS[src].id) != want
        {
            self.vals.edit_plain(SPECS[src].id, want);
        }
    }

    /// Movement of a locked drag in logical px since the last event.
    pub fn delta(&mut self, _dx: f32, dy: f32, m: Mods) -> Vec<Act> {
        let Some(Drag { param, v, px, axis }) = self.drag else {
            return Vec::new();
        };
        let fine = if m.ctrl { FINE } else { 1.0 };
        let moved = match axis {
            Axis::Up => -dy,
        };
        let v = (v + moved * fine / px).clamp(0.0, 1.0);
        self.vals.set(SPECS[param].id, v);
        self.drag = Some(Drag { param, v, px, axis });
        vec![Act::Repaint]
    }

    /// The press ends.
    pub fn up(&mut self) -> Vec<Act> {
        if let Some(drag) = self.drag.take() {
            self.vals.end(SPECS[drag.param].id);
        }
        self.pressed = None;
        vec![Act::Repaint]
    }

    /// The wheel over a control: `notches` up are positive.
    pub fn wheel(&mut self, x: f32, y: f32, notches: f32, m: Mods) -> Vec<Act> {
        let Some(h) = self.hit(x, y) else {
            return Vec::new();
        };
        if notches == 0.0 || !h.is_param() {
            return Vec::new();
        }
        let p = h.param;
        self.last = Some(p);
        self.claim(p);
        let sign = notches.signum() as i32;
        match h.kind {
            Kind::Cycle | Kind::Toggle | Kind::Stepper => self.step(p, sign, false),
            _ if view::is_int(p) => self.step(p, sign, false),
            _ => {
                let id = SPECS[p].id;
                let fine = if m.ctrl { FINE } else { 1.0 };
                let v = (self.vals.norm(id) + notches * WHEEL_STEP * fine).clamp(0.0, 1.0);
                self.vals.edit(id, v);
            }
        }
        vec![Act::Repaint]
    }
}

/// Rind's editor, as nih-plug's wrapper knows it.
pub struct RindEditor {
    params: Arc<RindParams>,
    #[cfg(feature = "embed")]
    scale: AtomicU32,
    /// The open editor's waker, for parameter changes.
    #[cfg(feature = "embed")]
    slot: os_plugkit::gui::Slot,
}

impl RindEditor {
    pub fn new(params: Arc<RindParams>) -> Self {
        RindEditor {
            params,
            #[cfg(feature = "embed")]
            scale: AtomicU32::new(1f32.to_bits()),
            #[cfg(feature = "embed")]
            slot: Default::default(),
        }
    }

    /// A parameter changed: the open editor looks again.
    fn notify(&self) {
        #[cfg(feature = "embed")]
        os_plugkit::gui::notify(&self.slot);
    }
}

impl Editor for RindEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        #[cfg(feature = "embed")]
        {
            let scale = f32::from_bits(self.scale.load(Ordering::Relaxed));
            match parent {
                // Honeydew gives the window as an X11 window id on every platform but macOS.
                #[cfg(not(target_os = "macos"))]
                ParentWindowHandle::X11Window(window) => Box::new(gui::open(
                    window,
                    scale,
                    self.params.clone(),
                    context,
                    &self.slot,
                )),
                // macOS sets no scale (sizes are points): the editor paints at its window's backing scale.
                #[cfg(target_os = "macos")]
                ParentWindowHandle::AppKitNsView(view) => {
                    let scale = os_plugkit::gui::backing_scale(view as usize);
                    Box::new(gui::open(
                        view as usize,
                        scale,
                        self.params.clone(),
                        context,
                        &self.slot,
                    ))
                }
                _ => {
                    let _ = (context, scale);
                    Box::new(())
                }
            }
        }
        #[cfg(not(feature = "embed"))]
        {
            let _ = (parent, context);
            Box::new(())
        }
    }

    fn size(&self) -> (u32, u32) {
        (view::W, view::H)
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        #[cfg(feature = "embed")]
        self.scale.store(_factor.to_bits(), Ordering::Relaxed);
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        self.notify();
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {
        self.notify();
    }

    fn param_values_changed(&self) {
        self.notify();
    }
}

#[cfg(feature = "embed")]
mod gui {
    use std::sync::Arc;

    use nih_plug::prelude::GuiContext;
    use os_plugkit::gui::{self as px, Button, Input, Window};
    use os_plugkit::vals::Vals;
    use os_ui::Canvas;

    use super::{Act, Btn, Logic, Mods};
    use crate::params::RindParams;
    use crate::view;

    /// The editor on the app thread.
    struct Ed {
        win: Window,
        canvas: Canvas,
        buf: Vec<u32>,
        scale: f32,
        size: (u32, u32),
        logic: Logic,
        dirty: bool,
        /// The fingerprint of the values as last painted: a tick repaints only when it moved.
        signature: u64,
    }

    /// Opens editor `parent` at `scale` device pixels per logical pixel.
    pub fn open(
        parent: px::Parent,
        scale: f32,
        params: Arc<RindParams>,
        ctx: Arc<dyn GuiContext>,
        slot: &px::Slot,
    ) -> Option<px::EditorThread> {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let size = (
            (view::W as f32 * scale).round() as u32,
            (view::H as f32 * scale).round() as u32,
        );
        px::spawn(parent, size, slot, move |win| {
            let Ok(canvas) = Canvas::new(size.0, size.1, scale) else {
                return;
            };
            let vals = Vals::new(&*params, params.live.clone(), Some(ctx));
            let signature = vals.signature();
            let ed = Ed {
                win,
                canvas,
                buf: Vec::new(),
                scale,
                size,
                logic: Logic::new(vals),
                dirty: true,
                signature,
            };
            ed.run();
        })
    }

    fn btn(b: Button) -> Btn {
        match b {
            Button::Left => Btn::Left,
            Button::Right => Btn::Right,
            Button::Middle => Btn::Middle,
        }
    }

    impl Ed {
        fn run(mut self) {
            while let Some(input) = self.win.next() {
                match input {
                    Input::Expose => self.dirty = true,
                    Input::Tick => self.poll(),
                    Input::Down {
                        x,
                        y,
                        btn: b,
                        dbl: _,
                    } => {
                        let acts =
                            self.logic
                                .down(x / self.scale, y / self.scale, btn(b), self.mods());
                        self.act(acts);
                    }
                    Input::Delta { dx, dy } => {
                        let acts = self
                            .logic
                            .delta(dx / self.scale, dy / self.scale, self.mods());
                        self.act(acts);
                    }
                    Input::Up { .. } => {
                        self.win.unlock();
                        let acts = self.logic.up();
                        self.act(acts);
                    }
                    Input::Wheel { x, y, notches } => {
                        let acts =
                            self.logic
                                .wheel(x / self.scale, y / self.scale, notches, self.mods());
                        self.act(acts);
                    }
                    Input::Move { .. } | Input::Leave | Input::Key { .. } | Input::Drop { .. } => {}
                }
                if self.dirty && self.win.idle() {
                    self.paint();
                }
            }
            os_plugkit::park!(self);
        }

        fn mods(&self) -> Mods {
            let (ctrl, _shift, alt) = self.win.mods();
            Mods { ctrl, alt }
        }

        /// Timer / parameter change: expire confirmed edits and repaint if a value moved.
        fn poll(&mut self) {
            let expired = self.logic.vals.expire();
            let signature = self.logic.vals.signature();
            if expired || signature != self.signature {
                self.signature = signature;
                self.dirty = true;
            }
        }

        fn act(&mut self, acts: Vec<Act>) {
            for a in acts {
                match a {
                    Act::Repaint => self.dirty = true,
                    Act::Lock => self.win.lock(),
                }
            }
        }

        fn paint(&mut self) {
            self.dirty = false;
            self.logic.paint(&mut self.canvas);
            self.signature = self.logic.vals.signature();
            self.buf.resize((self.size.0 * self.size.1) as usize, 0);
            self.canvas.copy_to_xrgb(&mut self.buf);
            self.win.blit(&self.buf, self.size.0, self.size.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids;

    /// The parameters must outlive the logic: `Vals` holds raw pointers into them, so every test binds them.
    fn logic() -> (Logic, Arc<RindParams>) {
        let params = Arc::new(RindParams::default());
        let vals = Vals::new(&*params, params.live.clone(), None);
        let mut l = Logic::new(vals);
        paint(&mut l);
        (l, params)
    }

    fn paint(l: &mut Logic) -> Canvas {
        let mut c = Canvas::new(view::W, view::H, 1.0).unwrap();
        l.paint(&mut c);
        c
    }

    fn tap(l: &mut Logic, r: os_ui::Rect) {
        l.down(r.cx(), r.cy(), Btn::Left, Mods::default());
        l.up();
        paint(l);
    }

    /// Taps the navigation control `n`.
    fn nav(l: &mut Logic, n: view::Nav) {
        let r = l.hits.iter().find(|h| h.kind == Kind::Nav(n)).unwrap().r;
        tap(l, r);
    }

    /// The centre of the control of parameter `p` on the current screen.
    fn at(l: &Logic, p: usize) -> (f32, f32) {
        let h = l
            .hits
            .iter()
            .find(|h| h.param == p && h.is_param())
            .unwrap_or_else(|| panic!("no control for {}", SPECS[p].id));
        (h.r.cx(), h.r.cy())
    }

    /// The name number `i` of the open list.
    fn item(l: &Logic, i: usize) -> os_ui::Rect {
        l.hits.iter().find(|h| h.kind == Kind::Item(i)).unwrap().r
    }

    /// Taps name number `i` of the open list.
    fn pick(l: &mut Logic, i: usize) {
        let r = item(l, i);
        tap(l, r);
    }

    fn live(params: &RindParams, p: usize) -> f32 {
        let mut snap = RindParams::defaults();
        params.snapshot(&mut snap);
        snap[p]
    }

    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
    };

    fn norm(l: &Logic, p: usize) -> f32 {
        l.vals.norm(SPECS[p].id)
    }

    #[test]
    fn every_screen_paints_something_and_the_buttons_switch_it() {
        let (mut l, _params) = logic();
        let wave = ids::OSC_BASE + ids::osc::WAVE;
        // The steps page belongs to the arpeggiator, the effect list to the FX tab.
        for n in [
            view::Nav::Part(1),
            view::Nav::Page(1),
            view::Nav::Fx(6),
            view::Nav::Tab(1),
            view::Nav::Part(0),
            view::Nav::Tab(0),
        ] {
            nav(&mut l, n);
            let c = paint(&mut l);
            let mut px = vec![0u32; (view::W * view::H) as usize];
            c.copy_to_xrgb(&mut px);
            let distinct: std::collections::HashSet<_> = px.iter().copied().collect();
            assert!(distinct.len() > 8, "{n:?} is blank");
        }
        assert_eq!(l.ui.fx, 6);
        assert_eq!(l.ui.page, 1);
        assert_eq!(l.ui.part, 0);
        at(&l, wave);
        nav(&mut l, view::Nav::Part(1));
        assert_eq!(l.ui.part, 1);
        assert!(
            !l.hits.iter().any(|h| h.param == wave && h.is_param()),
            "part B shows other oscillators"
        );
    }

    #[test]
    fn a_fader_is_set_where_it_is_pressed_and_a_middle_click_resets_it() {
        let (mut l, params) = logic();
        let h = *l
            .hits
            .iter()
            .find(|h| h.param == ids::MASTER_VOL && h.kind == Kind::Fader)
            .unwrap();
        let top = h.r.y + view::FADER_PAD;
        assert_eq!(
            l.down(h.r.cx(), top, Btn::Left, NONE),
            [Act::Repaint, Act::Lock]
        );
        assert!(l.drag.is_some());
        assert!((live(&params, ids::MASTER_VOL) - 1.0).abs() < 1e-3);
        // A fifth of the travel down.
        l.delta(0.0, 0.2 * (h.r.h - 2.0 * view::FADER_PAD), NONE);
        l.up();
        assert!(l.drag.is_none());
        let heard = live(&params, ids::MASTER_VOL);
        assert!((heard - 0.8).abs() < 1e-3, "{heard}");
        l.down(h.r.cx(), h.r.cy(), Btn::Middle, NONE);
        assert!((live(&params, ids::MASTER_VOL) - 0.7).abs() < 1e-4);
        // Alt-click resets too.
        l.down(h.r.cx(), top, Btn::Left, NONE);
        l.up();
        l.down(h.r.cx(), h.r.cy(), Btn::Left, Mods { alt: true, ..NONE });
        assert!((live(&params, ids::MASTER_VOL) - 0.7).abs() < 1e-4);
    }

    #[test]
    fn a_knob_drag_moves_with_the_cursor_and_a_fine_drag_a_quarter_as_far() {
        let (mut l, _params) = logic();
        let cutoff = ids::FILT_BASE + ids::filt::CUTOFF;
        let (x, y) = at(&l, cutoff);
        l.vals.edit(SPECS[cutoff].id, 0.5);
        l.down(x, y, Btn::Left, NONE);
        l.delta(0.0, -30.0, NONE);
        l.up();
        assert!(
            (norm(&l, cutoff) - 0.7).abs() < 1e-3,
            "{}",
            norm(&l, cutoff)
        );
        l.down(x, y, Btn::Left, NONE);
        l.delta(0.0, 30.0, Mods { ctrl: true, ..NONE });
        l.up();
        assert!(
            (norm(&l, cutoff) - 0.65).abs() < 1e-3,
            "{}",
            norm(&l, cutoff)
        );
    }

    #[test]
    fn a_short_selector_steps_on_a_tap_wraps_and_goes_back_on_a_right_click() {
        let (mut l, params) = logic();
        let (x, y) = at(&l, ids::MONO);
        assert_eq!(live(&params, ids::MONO), 0.0);
        l.down(x, y, Btn::Left, NONE);
        l.up();
        assert_eq!(live(&params, ids::MONO), 1.0);
        assert_eq!(l.ui.popup, None, "two names step, they do not open a list");
        l.down(x, y, Btn::Right, NONE);
        assert_eq!(live(&params, ids::MONO), 0.0);
        l.down(x, y, Btn::Right, NONE);
        assert_eq!(live(&params, ids::MONO), 1.0, "wraps below the first");
        l.down(x, y, Btn::Left, NONE);
        assert_eq!(live(&params, ids::MONO), 0.0, "wraps past the last");
    }

    #[test]
    fn a_long_selector_opens_a_list_and_a_name_picks_it() {
        let (mut l, params) = logic();
        let wave = ids::OSC_BASE + ids::osc::WAVE;
        let (x, y) = at(&l, wave);
        assert_eq!(live(&params, wave), 1.0);
        l.down(x, y, Btn::Left, NONE);
        l.up();
        paint(&mut l);
        assert_eq!(l.ui.popup, Some(wave));
        assert_eq!(live(&params, wave), 1.0, "opening changes nothing");
        // Every name is on the list.
        for i in 0..ids::WAVE_NAMES.len() {
            item(&l, i);
        }
        pick(&mut l, 4);
        assert_eq!(l.ui.popup, None);
        assert_eq!(live(&params, wave), 4.0);
        assert_eq!(l.last, Some(wave));
        // The list is closed: the control is back and a press on it opens the list again.
        let (x, y) = at(&l, wave);
        l.down(x, y, Btn::Left, NONE);
        assert_eq!(l.ui.popup, Some(wave));
    }

    #[test]
    fn a_press_outside_the_list_or_a_right_click_closes_it_without_a_change() {
        let (mut l, params) = logic();
        let wave = ids::OSC_BASE + ids::osc::WAVE;
        let (x, y) = at(&l, wave);
        let (fx, fy) = at(&l, ids::MASTER_VOL);
        l.down(x, y, Btn::Left, NONE);
        paint(&mut l);
        // A press on the master fader's place (under the shade) closes the list; the fader is not touched.
        let before = live(&params, ids::MASTER_VOL);
        assert!(l.down(fx, fy, Btn::Left, NONE).contains(&Act::Repaint));
        assert_eq!(l.ui.popup, None);
        assert!(l.drag.is_none());
        assert_eq!(live(&params, wave), 1.0);
        assert_eq!(live(&params, ids::MASTER_VOL), before);
        // A right click on a name closes it without picking.
        l.down(x, y, Btn::Left, NONE);
        paint(&mut l);
        let r = item(&l, 3);
        l.down(r.cx(), r.cy(), Btn::Right, NONE);
        assert_eq!(l.ui.popup, None);
        assert_eq!(live(&params, wave), 1.0);
        // The wheel does nothing under an open list.
        l.down(x, y, Btn::Left, NONE);
        paint(&mut l);
        assert!(l.wheel(x, y, 1.0, NONE).is_empty());
        assert_eq!(live(&params, wave), 1.0);
    }

    #[test]
    fn a_free_source_row_and_its_partner_share_the_source_and_a_fixed_row_claims_its_own() {
        let (mut l, params) = logic();
        let mat = |slot: usize, f: usize| ids::MAT_BASE + slot * ids::MAT_STRIDE + f;
        // Misc 1: slot 8 has the source list, slot 9 follows it.
        let (x, y) = at(&l, mat(8, ids::mat::SRC));
        l.down(x, y, Btn::Left, NONE);
        l.up();
        paint(&mut l);
        assert_eq!(l.ui.popup, Some(mat(8, ids::mat::SRC)));
        pick(&mut l, ids::msrc::ENV_3);
        assert_eq!(
            live(&params, mat(8, ids::mat::SRC)),
            ids::msrc::ENV_3 as f32
        );
        assert_eq!(
            live(&params, mat(9, ids::mat::SRC)),
            ids::msrc::ENV_3 as f32
        );
        // Touching the second row's target keeps the shared source.
        let (x, y) = at(&l, mat(9, ids::mat::DST));
        l.down(x, y, Btn::Left, NONE);
        l.up();
        paint(&mut l);
        let last = ids::TARGET_NAMES.len() - 1;
        pick(&mut l, last);
        assert_eq!(live(&params, mat(9, ids::mat::DST)), last as f32);
        assert_eq!(
            live(&params, mat(9, ids::mat::SRC)),
            ids::msrc::ENV_3 as f32
        );
        // Mod Env 1's first row (slot 0) is an envelope's: touching its amount claims the source.
        l.vals.edit_plain(SPECS[mat(0, ids::mat::SRC)].id, 0.0);
        let (x, y) = at(&l, mat(0, ids::mat::AMT));
        l.down(x, y, Btn::Left, NONE);
        l.up();
        assert_eq!(
            live(&params, mat(0, ids::mat::SRC)),
            ids::msrc::ENV_3 as f32
        );
    }

    #[test]
    fn a_switch_flips_and_the_wheel_holds_at_the_ends() {
        let (mut l, params) = logic();
        let (x, y) = at(&l, ids::ARP_ON);
        assert_eq!(live(&params, ids::ARP_ON), 0.0);
        l.down(x, y, Btn::Left, NONE);
        assert_eq!(live(&params, ids::ARP_ON), 1.0);
        l.down(x, y, Btn::Left, NONE);
        assert_eq!(live(&params, ids::ARP_ON), 0.0);
        let (mx, my) = at(&l, ids::ARP_MODE);
        l.wheel(mx, my, 1.0, NONE);
        assert_eq!(live(&params, ids::ARP_MODE), 1.0);
        l.wheel(mx, my, -5.0, NONE);
        l.wheel(mx, my, -5.0, NONE);
        assert_eq!(live(&params, ids::ARP_MODE), 0.0);
    }

    #[test]
    fn a_stepper_steps_by_its_halves_and_by_the_wheel() {
        let (mut l, params) = logic();
        let h = *l.hits.iter().find(|h| h.param == ids::ARP_OCT).unwrap();
        assert_eq!(h.kind, Kind::Stepper);
        assert_eq!(live(&params, ids::ARP_OCT), 1.0);
        l.down(h.r.right() - 2.0, h.r.cy(), Btn::Left, NONE);
        assert_eq!(live(&params, ids::ARP_OCT), 2.0);
        l.wheel(h.r.cx(), h.r.cy(), 1.0, NONE);
        assert_eq!(live(&params, ids::ARP_OCT), 3.0);
        l.down(h.r.x + 2.0, h.r.cy(), Btn::Left, NONE);
        assert_eq!(live(&params, ids::ARP_OCT), 2.0);
    }

    #[test]
    fn an_arp_step_is_dragged_and_a_hold_toggles_on_either_page() {
        let (mut l, params) = logic();
        let step = ids::ARP_STEP1 + 2;
        let (x, y) = at(&l, step);
        l.vals.edit(SPECS[step].id, 0.0);
        l.down(x, y, Btn::Left, NONE);
        l.delta(0.0, -60.0, NONE);
        l.up();
        assert!((norm(&l, step) - 0.5).abs() < 1e-3, "{}", norm(&l, step));
        let hold = ids::ARP_HOLD1 + 2;
        let (x, y) = at(&l, hold);
        let before = live(&params, hold);
        l.down(x, y, Btn::Left, NONE);
        assert_ne!(live(&params, hold), before);
        // The second page holds steps 9-16.
        nav(&mut l, view::Nav::Page(1));
        at(&l, ids::ARP_STEP1 + 10);
        at(&l, ids::ARP_TR1 + 10);
        at(&l, ids::ARP_HOLD1 + 10);
    }

    #[test]
    fn a_press_off_every_control_does_nothing() {
        let (mut l, params) = logic();
        assert!(l.down(1.0, 100.0, Btn::Left, NONE).is_empty());
        assert!(l.delta(0.0, -10.0, NONE).is_empty());
        assert_eq!(live(&params, ids::MASTER_VOL), 0.7);
    }
}
