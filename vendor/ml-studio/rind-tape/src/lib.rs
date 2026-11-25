// SPDX-License-Identifier: GPL-3.0-or-later
// Rind Tape: a tape machine in the ML Studio look. The sound (`dsp`) is after ChowTape by Jatin Chowdhury (GPL-3.0-or-later,
// https://github.com/jatinchowdhury18/AnalogTapeModel); the editor and the settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::{Fx, Unit, extra::Notes};
use ml_small_fx::{Card, Control, Mark, Setting, Spec, editor::Extension, params::FxParams};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::sync::Arc;

pub mod dsp;

pub type Effect = Fx<RindTape>;

const fn setting(id: &'static str, name: &'static str, range: (i32, i32), start: i32) -> Setting {
    Setting {
        id,
        name,
        range,
        start,
        magnet: None,
    }
}

pub static SETTINGS: [Setting; 10] = [
    setting("drive", "Drive", (0, 1000), 500),
    setting("sat", "Saturation", (0, 1000), 500),
    setting("bias", "Bias", (0, 1000), 500),
    setting("speed", "Speed", (10, 500), 150),
    setting("spacing", "Spacing", (0, 1000), 500),
    setting("gap", "Gap", (0, 1000), 0),
    setting("wow", "Wow", (0, 1000), 0),
    setting("flutter", "Flutter", (0, 1000), 0),
    setting("mix", "Mix", (0, 1000), 1000),
    Setting {
        id: "output",
        name: "Output",
        range: (-240, 120),
        start: 0,
        magnet: Some(0),
    },
];

const HEADINGS: [&str; 10] = [
    "DRIVE",
    "SATURATION",
    "BIAS",
    "SPEED",
    "SPACING",
    "GAP",
    "WOW",
    "FLUTTER",
    "MIX",
    "OUTPUT",
];

const fn card(n: usize, heading: &'static str) -> Card {
    let (col, row) = ((n % 5) as f32, (n / 5) as f32);
    Card {
        x0: 8. + 124. * col,
        x1: 124. + 124. * col,
        y0: 28. + 134. * row,
        y1: 153. + 134. * row,
        heading,
    }
}
const fn knob(n: usize, id: &'static str, bipolar: bool) -> Control {
    let (col, row) = ((n % 5) as f32, (n / 5) as f32);
    Control::Knob {
        id,
        cx: 66. + 124. * col,
        cy: 85. + 134. * row,
        r: 24.,
        caption: "",
        bipolar,
    }
}

pub static SPEC: Spec = Spec {
    name: "Rind Tape",
    word: "TAPE",
    about: "A tape machine after ChowTape: hysteresis saturation, head loss, wow and flutter.",
    settings: &SETTINGS,
    studio_settings: 10,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (636, 324),
    cards: &[
        card(0, HEADINGS[0]),
        card(1, HEADINGS[1]),
        card(2, HEADINGS[2]),
        card(3, HEADINGS[3]),
        card(4, HEADINGS[4]),
        card(5, HEADINGS[5]),
        card(6, HEADINGS[6]),
        card(7, HEADINGS[7]),
        card(8, HEADINGS[8]),
        card(9, HEADINGS[9]),
    ],
    controls: &[
        knob(0, "drive", false),
        knob(1, "sat", false),
        knob(2, "bias", false),
        knob(3, "speed", false),
        knob(4, "spacing", false),
        knob(5, "gap", false),
        knob(6, "wow", false),
        knob(7, "flutter", false),
        knob(8, "mix", false),
        knob(9, "output", true),
    ],
    display: None,
    mark: Mark {
        x: 620.,
        y: 306.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text,
    parse,
    sound: |_, _| unreachable!(),
    rings: false,
};

/// The head's spacing a stored value means, in microns: 0.1 to 20.
fn spacing_um(v: i32) -> f64 {
    0.1 * 200f64.powf(f64::from(v) / 1000.)
}
/// The playback gap a stored value means, in microns: 1 to 50.
fn gap_um(v: i32) -> f64 {
    50f64.powf(f64::from(v) / 1000.)
}

fn text(k: usize, v: i32) -> String {
    match k {
        3 => format!("{:.1} ips", f64::from(v) / 10.),
        4 => format!("{:.2} µm", spacing_um(v)),
        5 => format!("{:.1} µm", gap_um(v)),
        9 => format!("{:+.1} dB", f64::from(v) / 10.),
        _ => format!("{:.0}%", f64::from(v) / 10.),
    }
}

fn parse(k: usize, t: &str) -> Option<i32> {
    let n = t
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())?;
    let p = SETTINGS.get(k)?;
    let v = match k {
        3 | 9 => n * 10.,
        4 => (n.max(0.1) / 0.1).ln() / 200f64.ln() * 1000.,
        5 => n.max(1.).ln() / 50f64.ln() * 1000.,
        _ => n * 10.,
    };
    Some((v.round() as i32).clamp(p.range.0, p.range.1))
}

#[derive(Default)]
pub struct RindTape {
    machine: Option<dsp::Machine>,
    rate: f32,
}
impl RindTape {
    fn tape(p: &FxParams) -> dsp::Tape {
        let unit = |k: usize| f64::from(p.v(k)) / 1000.;
        dsp::Tape {
            drive: unit(0),
            sat: unit(1),
            bias: unit(2),
            speed: f64::from(p.v(3)) / 10.,
            spacing: spacing_um(p.v(4)),
            gap: gap_um(p.v(5)),
            wow: unit(6),
            flutter: unit(7),
            mix: unit(8),
            out: 10f64.powf(f64::from(p.v(9)) / 200.),
        }
    }
}
impl Unit for RindTape {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-tape";
    const CLASS: [u8; 16] = *b"RindTape00000001";
    const LATENCY: u32 = dsp::LATENCY;
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate;
        self.reset(p);
    }
    fn reset(&mut self, p: &FxParams) {
        let mut m = dsp::Machine::new(f64::from(self.rate.max(8000.)));
        m.set(Self::tape(p));
        self.machine = Some(m);
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        if self.machine.is_none() {
            self.reset(p);
        }
        if let Some(m) = &mut self.machine {
            m.set(Self::tape(p));
            m.process(l, r);
        }
    }
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[]))
    }
}

#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_texts_parse_back() {
        for (k, s) in SETTINGS.iter().enumerate() {
            for v in [s.range.0, s.start, s.range.1] {
                let back = parse(k, &text(k, v)).unwrap_or_else(|| panic!("{} {v}", s.id));
                // Texts are rounded, so allow a step of the shown precision.
                assert!(
                    (back - v).abs() <= 12,
                    "{}: {v} -> {} -> {back}",
                    s.id,
                    text(k, v)
                );
            }
        }
    }

    #[test]
    fn the_spec_is_laid_out() {
        assert_eq!(SPEC.settings.len(), 10);
        assert_eq!(SPEC.cards.len(), 10);
        assert_eq!(SPEC.controls.len(), 10);
        for (c, s) in SPEC.controls.iter().zip(&SETTINGS) {
            assert_eq!(c.id(), s.id);
        }
    }
}
