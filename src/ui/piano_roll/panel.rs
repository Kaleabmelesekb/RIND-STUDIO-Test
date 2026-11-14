//! The piano roll's tool panel, down the left of the roll as in Ableton's clip view: every note tool a button in view
//! (no `More...` menu to open first), with amount knobs for what takes one (Swing, Randomize, Velocity). The buttons run
//! the note menu's own operations ([`MoreOp`]) on the selected notes, or on all of them while none is selected.
//!
//! Drawn only in the running app (the unit tests place the roll by position).

use std::hash::{BuildHasher, RandomState};

use eframe::egui::{self, Align2, Color32, Id, Pos2, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use super::notes::{CHORDS, Slice, note};
use super::{MoreOp, PianoRoll};
use crate::metrics::{FontSize, Metrics};
use crate::song::Song;
use crate::ui::UiCtx;
use crate::ui::widgets::{self, Control, ParamMenu, knob};

/// The panel's width open and as a rail, in U.
const OPEN_W: f32 = 9.2;
const RAIL_W: f32 = 1.25;
/// A button's height, in U.
const BTN_H: f32 = 1.35;
/// The gap between buttons and the panel's padding, in U.
const GAP: f32 = 0.25;
/// A knob's cell, in U.
const KNOB_W: f32 = 4.2;
const KNOB_H: f32 = 4.6;

/// What the panel keeps: whether it shows and its amount knobs' values (0 to 1).
#[derive(Clone, Copy, Debug)]
pub(super) struct Tools {
    pub open: bool,
    /// Randomize's amount: how far a velocity may move (1 is what the note menu's `Random Velocity` always did).
    pub random: f32,
    /// The velocity `Set` gives the notes.
    pub velocity: f32,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            open: true,
            random: 1.0,
            velocity: 0.8,
        }
    }
}

/// How a button looks.
#[derive(Clone, Copy)]
struct Look {
    text: Color32,
    dim: Color32,
    accent: Color32,
    well: Color32,
    button: Color32,
    hover: Color32,
    line: Color32,
    px: f32,
}

impl Look {
    fn of(cx: &UiCtx) -> Self {
        let c = &cx.theme.colors;
        let text = c.text.to_color32();
        Self {
            text,
            dim: text.gamma_multiply(0.6),
            accent: c.accent.to_color32(),
            well: c.control_fill.to_color32(),
            button: c.module_fill.to_color32(),
            hover: c.menu_fill.to_color32(),
            line: text.gamma_multiply(0.18),
            px: 1.0 / cx.m.ppp(),
        }
    }
}

/// The panel's width, in points.
pub(super) fn width(open: bool, m: &Metrics) -> f32 {
    m.u(if open { OPEN_W } else { RAIL_W })
}

impl PianoRoll {
    /// The width the panel takes now, in points.
    pub(crate) fn tools_width(&self, m: &Metrics) -> f32 {
        width(self.tools.open, m)
    }

    /// Draws the panel in `rect` and does what its buttons ask on the selected notes (or all of them) of the clip.
    pub(crate) fn tool_panel(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        cx: &UiCtx,
        song: &mut Song,
        covered: bool,
    ) {
        let (m, look) = (cx.m, Look::of(cx));
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, look.well);
        painter.line_segment(
            [rect.right_top(), rect.right_bottom()],
            Stroke::new(look.px, look.line),
        );
        let sense = if covered {
            Sense::hover()
        } else {
            Sense::click()
        };
        // The header: the collapse chevron and the title.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), m.u(1.5)));
        let toggle = ui.interact(header, Id::new("roll tools toggle"), sense);
        if toggle.clicked() {
            self.tools.open ^= true;
        }
        let chevron = pos2(rect.left() + m.u(0.7), header.center().y);
        let arm = m.u(0.22);
        let (a, b) = if self.tools.open {
            (vec2(-arm, -arm * 0.5), vec2(arm, -arm * 0.5))
        } else {
            (vec2(-arm * 0.5, -arm), vec2(-arm * 0.5, arm))
        };
        let tip = if self.tools.open {
            vec2(0.0, arm * 0.6)
        } else {
            vec2(arm * 0.7, 0.0)
        };
        let ink = if widgets::hovered(ui, &toggle) {
            look.text
        } else {
            look.dim
        };
        let stroke = Stroke::new(1.5 * look.px, ink);
        painter.line_segment([chevron + a, chevron + tip], stroke);
        painter.line_segment([chevron + b, chevron + tip], stroke);
        if !self.tools.open {
            for (i, c) in "TOOLS".chars().enumerate() {
                let at = pos2(rect.center().x, rect.top() + m.u(2.2 + 0.7 * i as f32));
                painter.text(
                    at,
                    Align2::CENTER_CENTER,
                    c,
                    m.font_id(FontSize::Pt8),
                    look.dim,
                );
            }
            return;
        }
        painter.text(
            pos2(rect.left() + m.u(1.4), header.center().y),
            Align2::LEFT_CENTER,
            "NOTE TOOLS",
            m.font_id(FontSize::Pt10),
            look.text,
        );
        let body = rect.with_min_y(header.bottom());
        painter.line_segment(
            [body.left_top(), body.right_top()],
            Stroke::new(look.px, look.line),
        );

        let events = self.clip(song).map_or(&[][..], |c| &c.events);
        let notes = events.iter().filter(|e| note(e).is_some()).count();
        let chosen = self.selected.iter().filter(|&&s| s).count();
        let swing = (song.channels.get(self.channel))
            .and_then(|c| c.lanes.get(self.lane))
            .map_or(0.0, |l| l.swing as f32);
        let scope = match chosen {
            0 => format!("All {notes} notes"),
            n => format!("{n} of {notes} selected"),
        };

        let mut ops: Vec<MoreOp> = Vec::new();
        let mut swung = None;
        let pad = m.u(GAP);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body.shrink2(vec2(pad, 0.0)))
                .id_salt("roll tools"),
        );
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt("roll tools scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let w = ui.available_width();
                ui.add_space(pad);
                ui.painter().text(
                    ui.cursor().min + vec2(0.0, m.u(0.4)),
                    Align2::LEFT_CENTER,
                    &scope,
                    m.font_id(FontSize::Pt8),
                    look.dim,
                );
                ui.add_space(m.u(0.9));
                let mut grid = Grid {
                    n: 0,
                    w,
                    pad,
                    m,
                    look,
                };

                grid.section(ui, "Notes");
                for pair in [
                    [(MoreOp::Reverse, "Reverse"), (MoreOp::Flip, "Flip")],
                    [(MoreOp::Legato, "Legato"), (MoreOp::ToggleSlide, "Slide")],
                    [
                        (MoreOp::Stretch(true), "Stretch x2"),
                        (MoreOp::Stretch(false), "Stretch /2"),
                    ],
                    [
                        (MoreOp::DoubleUp, "Octave +"),
                        (MoreOp::DoubleDown, "Octave -"),
                    ],
                    [
                        (MoreOp::Strum(true), "Strum Up"),
                        (MoreOp::Strum(false), "Strum Down"),
                    ],
                    [
                        (MoreOp::Fade(true), "Fade In"),
                        (MoreOp::Fade(false), "Fade Out"),
                    ],
                    [
                        (MoreOp::SnapToScale, "Snap to Scale"),
                        (MoreOp::GuessScale, "Guess Scale"),
                    ],
                    [
                        (MoreOp::Humanize, "Humanize"),
                        (MoreOp::SelectAll, "Select All"),
                    ],
                ] {
                    grid.row(ui, &pair, covered, &mut ops);
                }

                grid.section(ui, "Transpose");
                let steps = [
                    (MoreOp::Shift(-12), "-12"),
                    (MoreOp::Shift(-1), "-1"),
                    (MoreOp::Shift(1), "+1"),
                    (MoreOp::Shift(12), "+12"),
                ];
                grid.strip(ui, &steps, covered, &mut ops);

                grid.section(ui, "Length");
                let lengths = [
                    (MoreOp::Length(32), "1/16"),
                    (MoreOp::Length(64), "1/8"),
                    (MoreOp::Length(128), "1/4"),
                    (MoreOp::Length(256), "1/2"),
                    (MoreOp::Length(512), "1"),
                ];
                grid.strip(ui, &lengths, covered, &mut ops);

                grid.section(ui, "Slice");
                let slices = [2u8, 3, 4, 8].map(|n| {
                    let label: &'static str = match n {
                        2 => "2",
                        3 => "3",
                        4 => "4",
                        _ => "8",
                    };
                    (MoreOp::Slice(Slice::Into(n)), label)
                });
                grid.strip(ui, &slices, covered, &mut ops);

                grid.section(ui, "Amounts");
                // Two knobs a row: Swing (the channel's, live) and Randomize's amount; then Velocity.
                let cell = vec2(m.u(KNOB_W), m.u(KNOB_H));
                let row_gap = (w - 2.0 * cell.x).max(0.0) / 3.0;
                let (r, _) = ui.allocate_exact_size(vec2(w, cell.y), Sense::hover());
                let spot = |i: f32| spot_in(r, cell, row_gap, i);
                let tint = look.accent;
                let mut v = swing;
                let control = |name: &'static str, rect: Rect, default: f32| Control {
                    menu: ParamMenu::Unbound,
                    ..Control::new(Id::new(("roll tool knob", name)), rect, name, tint, default)
                };
                let percent = |v: f32| format!("{}%", (v * 100.0).round());
                if knob(ui, cx, &control("Swing", spot(0.0), 0.0), &mut v, percent).changed() {
                    swung = Some((v.clamp(0.0, 1.0) * 100.0).round() as u8);
                }
                let mut a = self.tools.random;
                if knob(ui, cx, &control("Amount", spot(1.0), 1.0), &mut a, percent).changed() {
                    self.tools.random = a.clamp(0.0, 1.0);
                }
                grid.row(
                    ui,
                    &[(MoreOp::RandomVelocity, "Randomize")],
                    covered,
                    &mut ops,
                );
                let (r, _) = ui.allocate_exact_size(vec2(w, cell.y), Sense::hover());
                let mut v = self.tools.velocity;
                let shown = |v: f32| format!("{}", (v * 127.0).round().max(1.0));
                if knob(
                    ui,
                    cx,
                    &control("Velocity", spot_in(r, cell, row_gap, 0.0), 0.8),
                    &mut v,
                    shown,
                )
                .changed()
                {
                    self.tools.velocity = v.clamp(0.0, 1.0);
                }
                // `Set` beside it gives the notes that velocity.
                let set = Rect::from_min_size(
                    pos2(
                        spot_in(r, cell, row_gap, 1.0).left(),
                        r.center().y - m.u(BTN_H) / 2.0,
                    ),
                    vec2(cell.x, m.u(BTN_H)),
                );
                let velocity = (self.tools.velocity * 127.0).round().clamp(1.0, 127.0) as u8;
                if grid.button(ui, set, "Set", look.accent, covered) {
                    ops.push(MoreOp::SetVelocity(velocity));
                }

                grid.section(ui, "Chords");
                let stamps: Vec<(MoreOp, &'static str)> = CHORDS
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| (MoreOp::Chord(i as u8), *name))
                    .collect();
                for pair in stamps.chunks(2) {
                    grid.row(ui, pair, covered, &mut ops);
                }
                ui.add_space(m.u(1.0));
            });

        let at = rect.left_top();
        let seed = RandomState::new().hash_one(ui.input(|i| i.time).to_bits());
        let mut actions = Vec::new();
        if let Some(pct) = swung {
            self.more(MoreOp::Swing(pct), (at, seed), song, &mut actions);
        }
        for op in ops {
            self.more(op, (at, seed), song, &mut actions);
        }
        self.pending.extend(actions);
    }
}

/// Knob cell `i` of a row `r` of two.
fn spot_in(r: Rect, cell: egui::Vec2, gap: f32, i: f32) -> Rect {
    Rect::from_min_size(pos2(r.left() + gap * (i + 1.0) + cell.x * i, r.top()), cell)
}

/// The panel's column layout: a section's header, and buttons two to a row or in a strip.
struct Grid<'a> {
    /// Buttons drawn so far: each one's id, which a scroll does not move.
    n: u32,
    w: f32,
    pad: f32,
    m: &'a Metrics,
    look: Look,
}

impl Grid<'_> {
    fn section(&mut self, ui: &mut egui::Ui, title: &str) {
        let m = self.m;
        ui.add_space(m.u(0.5));
        let (r, _) = ui.allocate_exact_size(vec2(self.w, m.u(0.8)), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            title.to_uppercase(),
            m.font_id(FontSize::Pt8),
            self.look.dim,
        );
        ui.painter().line_segment(
            [r.left_bottom(), r.right_bottom()],
            Stroke::new(self.look.px, self.look.line),
        );
        ui.add_space(self.pad);
    }

    /// One button in `rect`; true when tapped.
    fn button(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        label: &str,
        ink: Color32,
        covered: bool,
    ) -> bool {
        self.n += 1;
        let m = self.m;
        let sense = if covered {
            Sense::hover()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, Id::new(("roll tool", self.n)), sense);
        let fill = if response.is_pointer_button_down_on() {
            self.look.accent.gamma_multiply(0.55)
        } else if widgets::hovered(ui, &response) {
            self.look.hover
        } else {
            self.look.button
        };
        ui.painter().rect_filled(rect, m.u(0.14), fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            m.font_id(FontSize::Pt8),
            if ink == self.look.accent {
                self.look.text
            } else {
                ink
            },
        );
        response.clicked()
    }

    /// One or two buttons side by side across the panel (an empty label leaves its place empty).
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        items: &[(MoreOp, &str)],
        covered: bool,
        ops: &mut Vec<MoreOp>,
    ) {
        let m = self.m;
        let (r, _) = ui.allocate_exact_size(vec2(self.w, m.u(BTN_H) + self.pad), Sense::hover());
        let cell = (self.w - self.pad) / 2.0;
        for (i, &(op, label)) in items.iter().enumerate() {
            if label.is_empty() {
                continue;
            }
            let at = pos2(r.left() + i as f32 * (cell + self.pad), r.top());
            let rect = Rect::from_min_size(at, vec2(cell, m.u(BTN_H)));
            if self.button(ui, rect, label, self.look.text, covered) {
                ops.push(op);
            }
        }
    }

    /// `items` as equal buttons across one row.
    fn strip(
        &mut self,
        ui: &mut egui::Ui,
        items: &[(MoreOp, &str)],
        covered: bool,
        ops: &mut Vec<MoreOp>,
    ) {
        let m = self.m;
        let (r, _) = ui.allocate_exact_size(vec2(self.w, m.u(BTN_H) + self.pad), Sense::hover());
        let n = items.len() as f32;
        let gap = m.u(0.12);
        let cell = (self.w - gap * (n - 1.0)) / n;
        for (i, &(op, label)) in items.iter().enumerate() {
            let at: Pos2 = pos2(r.left() + i as f32 * (cell + gap), r.top());
            let rect = Rect::from_min_size(at, vec2(cell, m.u(BTN_H)));
            if self.button(ui, rect, label, self.look.text, covered) {
                ops.push(op);
            }
        }
    }
}
