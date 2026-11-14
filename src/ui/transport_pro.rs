//! The control bar's Ableton-style extras, drawn only in the running app (the unit tests place everything by position,
//! so the bar there is the plain one): grouped button wells behind the transport, and, left of REC, the song position
//! as `bar . beat . sixteenth` over the clock time, a metronome toggle, `TAP` tempo and the time signature. The parts
//! that fit the free room beside the bar's own slots show, in that order, and the rest wait for a wider window.
//!
//! The song position and the time are read from the playhead tick; the time assumes the song's tempo throughout.

use eframe::egui::{
    self, Align2, Color32, Id, Painter, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2,
};

use crate::metrics::{FontSize, Metrics};
use crate::song::{PPQ, TimeSig};
use crate::ui::UiCtx;
use crate::ui::tempo::TapTempo;
use crate::ui::widgets;

/// What the bar shows besides what its slots take from the song: set by the app each frame.
#[derive(Clone, Copy, Debug)]
pub struct BarExtras {
    /// The metronome flag: the engine clicks while the transport plays.
    pub metronome: bool,
    pub time_sig: TimeSig,
    /// The playhead, in song ticks.
    pub tick: u64,
    pub bpm: f64,
}

impl Default for BarExtras {
    fn default() -> Self {
        Self {
            metronome: false,
            time_sig: TimeSig::default(),
            tick: 0,
            bpm: 120.0,
        }
    }
}

/// What a tap on the extras asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BarAsk {
    /// The metronome button was tapped.
    pub metronome: bool,
    /// The fourth `TAP` of a group: the tempo it gives.
    pub tempo: Option<f64>,
    /// The time signature or the position was tapped: the tempo dialog holds the signature.
    pub tempo_dialog: bool,
}

/// `tick` as `(bar, beat, sixteenth)`, each counted from 1.
pub fn position(tick: u64, time_sig: TimeSig) -> (u64, u64, u64) {
    let ppq = u64::from(PPQ);
    let beats_per_bar = u64::from(time_sig.bar_beats());
    let beat = tick / ppq;
    let sixteenth = (tick % ppq) / (ppq / 4).max(1);
    (
        beat / beats_per_bar + 1,
        beat % beats_per_bar + 1,
        sixteenth + 1,
    )
}

/// `tick` as `m:ss.d` at a steady `bpm`.
pub fn clock(tick: u64, bpm: f64) -> String {
    let seconds = tick as f64 / f64::from(PPQ) * 60.0 / bpm.max(1.0);
    let tenths = (seconds * 10.0).floor() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// The song position's text: `  12. 3. 4`.
pub fn position_text(tick: u64, time_sig: TimeSig) -> String {
    let (bar, beat, sixteenth) = position(tick, time_sig);
    format!("{bar}. {beat}. {sixteenth}")
}

/// A flat rounded well, as the Ableton transport groups its buttons: darker than the bar, with a faint outline.
pub fn well(painter: &Painter, rect: Rect, text: Color32, m: &Metrics) {
    let radius = m.u(0.18);
    painter.rect_filled(rect, radius, Color32::from_black_alpha(70));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0 / m.ppp(), text.gamma_multiply(0.14)),
        egui::StrokeKind::Inside,
    );
}

/// A readout well: a dark display with its text centred.
fn lcd(painter: &Painter, rect: Rect, text: Color32, m: &Metrics) {
    let radius = m.u(0.12);
    painter.rect_filled(rect, radius, Color32::from_black_alpha(130));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0 / m.ppp(), text.gamma_multiply(0.14)),
        egui::StrokeKind::Inside,
    );
}

/// A metronome: a tapering case with its pendulum.
fn metronome_icon(painter: &Painter, center: Pos2, size: f32, ink: Color32, px: f32) {
    let (w, h) = (0.34 * size, 0.46 * size);
    let case = vec![
        center + vec2(-0.55 * w, h),
        center + vec2(0.55 * w, h),
        center + vec2(0.22 * w, -h),
        center + vec2(-0.22 * w, -h),
    ];
    painter.add(Shape::closed_line(case, Stroke::new(1.5 * px, ink)));
    let base = center + vec2(0.0, 0.7 * h);
    let tip = center + vec2(0.4 * w, -0.85 * h);
    painter.line_segment([base, tip], Stroke::new(1.5 * px, ink));
    painter.circle_filled(base + (tip - base) * 0.55, 0.11 * size, ink);
}

/// Draws the extras that fit in `zone` (the free room left of REC, right of the keyboard button) and returns the part
/// of `zone` they leave for the scale name, with what the taps asked.
pub fn extras(
    ui: &mut egui::Ui,
    cx: &UiCtx,
    zone: Rect,
    now: f64,
    state: &BarExtras,
    tap: &mut TapTempo,
    playing: bool,
) -> (Rect, BarAsk) {
    let (c, m) = (&cx.theme.colors, cx.m);
    let text = c.text.to_color32();
    let accent = c.accent.to_color32();
    let px = 1.0 / m.ppp();
    let gap = m.u(0.2);
    let height = (zone.height() - m.u(0.5)).clamp(0.0, m.u(1.25));
    let top = zone.center().y - height / 2.0;
    let mut ask = BarAsk::default();
    // The widest first-to-last order of what shows; each takes its width and the gap before it.
    let mut right = zone.right() - m.u(0.3);
    let mut take = |w: f32| -> Option<Rect> {
        let left = right - w;
        (left >= zone.left() + m.u(2.5)).then(|| {
            right = left - gap;
            Rect::from_min_size(pos2(left, top), vec2(w, height))
        })
    };

    // The song position over the clock: the bar's LCD.
    if let Some(r) = take(m.u(4.6)) {
        lcd(ui.painter(), r, text, m);
        let (upper, lower) = (
            Rect::from_min_max(r.min, pos2(r.right(), r.center().y)),
            Rect::from_min_max(pos2(r.left(), r.center().y), r.max),
        );
        let big = m.font_id(FontSize::Pt10);
        let small = m.font_id(FontSize::Pt6);
        let at = Align2::CENTER_CENTER;
        let ink = if playing { accent } else { text };
        ui.painter().text(
            pos2(upper.center().x, upper.center().y + m.u(0.05)),
            at,
            position_text(state.tick, state.time_sig),
            big,
            ink,
        );
        ui.painter().text(
            pos2(lower.center().x, lower.center().y - m.u(0.05)),
            at,
            clock(state.tick, state.bpm),
            small,
            text.gamma_multiply(0.65),
        );
        let response = ui.interact(r, Id::new(("transport", "position")), Sense::click());
        if response.clicked() {
            ask.tempo_dialog = true;
        }
    }
    // The metronome.
    if let Some(r) = take(m.u(1.5)) {
        let response = ui.interact(r, Id::new(("transport", "metronome")), Sense::click());
        let hot = widgets::hovered(ui, &response);
        let fill = if state.metronome {
            accent
        } else if hot {
            Color32::from_white_alpha(30)
        } else {
            Color32::from_white_alpha(14)
        };
        ui.painter().rect_filled(r, m.u(0.14), fill);
        let ink = if state.metronome {
            c.control_fill.to_color32()
        } else {
            text
        };
        metronome_icon(ui.painter(), r.center(), r.height(), ink, px);
        if response.clicked() {
            ask.metronome = true;
        }
    }
    // TAP tempo.
    if let Some(r) = take(m.u(1.6)) {
        let response = ui.interact(r, Id::new(("transport", "tap")), Sense::click());
        let hot = widgets::hovered(ui, &response);
        let fill = if response.is_pointer_button_down_on() {
            accent.gamma_multiply(0.6)
        } else if hot {
            Color32::from_white_alpha(30)
        } else {
            Color32::from_white_alpha(14)
        };
        ui.painter().rect_filled(r, m.u(0.14), fill);
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            "TAP",
            m.font_id(FontSize::Pt8),
            text,
        );
        if response.clicked() {
            ask.tempo = tap.tap(now);
        }
    }
    // The time signature.
    if let Some(r) = take(m.u(1.6)) {
        lcd(ui.painter(), r, text, m);
        let sig = format!("{}/4", state.time_sig.bar_beats());
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            sig,
            m.font_id(FontSize::Pt8),
            text,
        );
        let response = ui.interact(r, Id::new(("transport", "sig")), Sense::click());
        if response.clicked() {
            ask.tempo_dialog = true;
        }
    }
    (zone.with_max_x((right + gap).max(zone.left())), ask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_counts_bars_beats_and_sixteenths_from_one() {
        let sig = TimeSig::default();
        let ppq = u64::from(PPQ);
        assert_eq!(position(0, sig), (1, 1, 1));
        assert_eq!(position(ppq / 4, sig), (1, 1, 2));
        assert_eq!(position(ppq, sig), (1, 2, 1));
        assert_eq!(position(4 * ppq, sig), (2, 1, 1));
        assert_eq!(position(4 * ppq + 3 * ppq + 3 * ppq / 4, sig), (2, 4, 4));
        let three = TimeSig {
            beats_per_bar: 3,
            ..sig
        };
        assert_eq!(position(3 * ppq, three), (2, 1, 1));
        assert_eq!(position_text(4 * ppq, sig), "2. 1. 1");
    }

    #[test]
    fn the_clock_reads_minutes_seconds_and_tenths_at_the_tempo() {
        let ppq = u64::from(PPQ);
        assert_eq!(clock(0, 120.0), "0:00.0");
        // A beat is half a second at 120.
        assert_eq!(clock(ppq, 120.0), "0:00.5");
        assert_eq!(clock(2 * 125 * ppq, 120.0), "2:05.0");
    }
}
