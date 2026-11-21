// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Lemon Soft Clipper: Lemon Studio's soft clipper (a recreation of FL's Fruity Soft Clipper) as a plugin. The sound is
//! `os_engine::effects::soft_clipper`; the editor has the two knobs on one card and the transfer curve in a well.
//!
// spec: plugins/lemon-soft-clipper.md, plugins/fruity-soft-clipper.md

use nih_plug::prelude::*;
use os_engine::effects::soft_clipper::{self as dsp, Settings, SoftClipper};
use os_ui::{Canvas, Rect};

use crate::quality::{Oversampler, OVERSAMPLING};
use crate::params::{db_text, parse_db};
use crate::{view, Card, Control, Display, Kind, Mark, Setting, SmallFx, Sound, Spec};

/// The settings in the original's order (spec §5).
const QUALITY: usize = dsp::PARAMS;
const KNEE: usize = QUALITY + 1;
const CEILING: usize = QUALITY + 2;
pub const SETTINGS: [Setting; dsp::PARAMS + 3] = [
    Setting { id: "threshold", name: "Threshold", range: dsp::RANGES[dsp::THRESHOLD], start: dsp::DEFAULTS[dsp::THRESHOLD], magnet: None },
    Setting { id: "post", name: "Post gain", range: dsp::RANGES[dsp::POST], start: dsp::DEFAULTS[dsp::POST], magnet: Some(128) },
    Setting { id: "quality", name: "Oversampling", range: (0, 2), start: 0, magnet: None },
    Setting { id: "knee", name: "Smooth knee", range: (0, 1), start: 0, magnet: None },
    Setting { id: "ceiling", name: "Output ceiling", range: (0, 1), start: 0, magnet: None },
];

/// Editor layout (spec §2), as the original's: the two knobs over the wordmark at the left, the curve at the right.
const CARDS: [Card; 3] = [
    Card { x0: 8.0, x1: 156.0, y0: 28.0, y1: 104.0, heading: "CLIPPER" },
    Card { x0: 162.0, x1: 312.0, y0: 28.0, y1: 152.0, heading: "CURVE" },
    Card { x0: 8.0, x1: 312.0, y0: 167.0, y1: 224.0, heading: "" },
];

const CONTROLS: [Control; 5] = [
    Control::Knob { id: "threshold", cx: 46.0, cy: 62.0, r: 20.0, caption: "THRESHOLD", bipolar: false },
    Control::Knob { id: "post", cx: 118.0, cy: 62.0, r: 20.0, caption: "POST GAIN", bipolar: false },
    Control::Choice { id: "quality", x: 18.0, top: 172.0, names: &OVERSAMPLING },
    Control::Choice { id: "knee", x: 97.0, top: 172.0, names: &["Classic", "Smooth"] },
    Control::Choice { id: "ceiling", x: 203.0, top: 172.0, names: &["Free", "0 dBFS"] },
];

pub const SPEC: &Spec = &Spec {
    name: "ML Soft Clipper",
    word: "SOFT CLIPPER",
    about: "ML Studio's soft clipper",
    settings: &SETTINGS,
    studio_settings: 3,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (320, 232),
    cards: &CARDS,
    controls: &CONTROLS,
    display: Some(Display { r: Rect::new(170.0, 36.0, 134.0, 108.0), draw: curve }),
    mark: Mark { x: 82.0, y: 114.0, anchor: 0.5, stacked: true },
    hint: None,
    labels: &[],
    text,
    parse,
    sound,
    rings: false,
};

/// Value texts (spec §3, Lemon Studio's choice): both as dB, the threshold of its level and the post gain of its gain.
fn text(k: usize, v: i32) -> String {
    match k {
        QUALITY => OVERSAMPLING[v.clamp(0, 2) as usize].into(),
        KNEE => if v == 0 { "Classic" } else { "Smooth" }.into(),
        CEILING => if v == 0 { "Off" } else { "0 dBFS" }.into(),
        dsp::THRESHOLD => db_text(dsp::threshold_level(v)),
        _ => db_text(dsp::post_gain(v)),
    }
}

fn parse(k: usize, s: &str) -> Option<i32> {
    match k {
        QUALITY => OVERSAMPLING.iter().position(|name| name.eq_ignore_ascii_case(s.trim())).map(|k| k as i32),
        KNEE => match s.trim().to_ascii_lowercase().as_str() {
            "classic" | "0" => Some(0),
            "smooth" | "1" => Some(1),
            _ => None,
        },
        CEILING => match s.trim().to_ascii_lowercase().as_str() {
            "free" | "off" => Some(0),
            "0 dbfs" | "on" => Some(1),
            _ => None,
        },
        dsp::THRESHOLD => parse_db(SETTINGS[k].range, s, dsp::threshold_level),
        _ => parse_db(SETTINGS[k].range, s, dsp::post_gain),
    }
}

fn settings(v: &[i32]) -> Settings {
    Settings { v: std::array::from_fn(|k| v[k]) }
}

/// The curve the settings make (input -2..2 across, output -1.25..1.25 up), post gain included.
fn curve(c: &mut Canvas, r: Rect, v: &[i32]) {
    let mut d = SoftClipper::with_settings(&settings(v));
    d.set_smooth_knee(v.get(KNEE).copied() == Some(1));
    let ceiling = v.get(CEILING).copied() == Some(1);
    view::transfer(c, r, 2.0, 1.25, |x| {
        let sample = d.sample(x);
        if ceiling { sample.clamp(-1.0, 1.0) } else { sample }
    });
}

struct Clip {
    shaper: SoftClipper,
    oversampling: Oversampler,
    ceiling: bool,
}

impl Sound for Clip {
    fn set(&mut self, v: &[i32]) {
        self.shaper.set(&settings(v));
        self.shaper.set_smooth_knee(v.get(KNEE).copied() == Some(1));
        self.oversampling.set(v.get(QUALITY).copied().unwrap_or_default());
        self.ceiling = v.get(CEILING).copied() == Some(1);
    }

    fn set_sample_rate(&mut self, _rate: u32) {
        self.reset();
    }

    fn reset(&mut self) {
        self.oversampling.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.oversampling.process(left, right, |sample| self.shaper.sample(sample));
        if self.ceiling {
            for sample in left.iter_mut().chain(right) {
                *sample = sample.clamp(-1.0, 1.0);
            }
        }
    }
}

fn sound(_rate: u32, v: &[i32]) -> Box<dyn Sound> {
    let mut clip = Clip {
        shaper: SoftClipper::with_settings(&settings(v)),
        oversampling: Oversampler::default(),
        ceiling: false,
    };
    clip.set(v);
    Box::new(clip)
}

pub struct SoftClipperKind;

impl Kind for SoftClipperKind {
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "org.openstudio.lemon-soft-clipper";
    const CLAP_DESCRIPTION: &'static str = "ML Studio's soft clipper";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Distortion, ClapFeature::Stereo];
    const VST3_CLASS_ID: [u8; 16] = *b"LemonStudioClipr";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

pub type LemonSoftClipper = SmallFx<SoftClipperKind>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_texts_and_typed_values() {
        // FL's Default preset: threshold 100 (-2.1 dB), post gain at unity.
        assert_eq!(text(dsp::THRESHOLD, 100), "-2.1dB");
        assert_eq!(text(dsp::THRESHOLD, 64), "-6.0dB");
        assert_eq!(text(dsp::THRESHOLD, 1), "-42.1dB");
        assert_eq!(text(dsp::POST, 128), "0.0dB");
        assert_eq!(text(dsp::POST, 0), "-INFdB");
        assert_eq!(text(dsp::POST, 160), "5.6dB");
        assert_eq!(parse(dsp::THRESHOLD, "-6"), Some(64));
        assert_eq!(parse(dsp::POST, "0dB"), Some(128));
        assert_eq!(parse(dsp::POST, "-inf"), Some(0));
        assert_eq!(parse(dsp::THRESHOLD, "-inf"), None);
    }

    #[test]
    fn the_plugin_plays_the_engines_soft_clipper() {
        let v = [64, 150];
        let mut s = sound(48000, &v);
        let mut l = vec![0.25f32, 0.6, -0.9, 1.5];
        let mut r = vec![-2.0f32, 0.1, 0.55, 0.0];
        let (l0, r0) = (l.clone(), r.clone());
        s.process(&mut l, &mut r);
        let d = SoftClipper::with_settings(&Settings { v });
        for (a, b) in l0.iter().zip(&l).chain(r0.iter().zip(&r)) {
            assert_eq!(d.sample(*a).to_bits(), b.to_bits());
        }
        // Stored values outside the ranges play as stored (threshold 0: the curve from 0).
        s.set(&[0, 128]);
        let mut l = vec![1.0f32];
        let mut r = vec![0.0f32];
        s.process(&mut l, &mut r);
        assert_eq!(l[0], (1.0 - (-1.0f64).exp()) as f32);
    }
}
