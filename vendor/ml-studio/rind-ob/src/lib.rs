// SPDX-License-Identifier: GPL-3.0-or-later
// Rind OB: a two-oscillator analogue-style polysynth with OB-Xf's engine (`cpp/`: the Surge Synth Team's OB-Xf, GPL-3.0-or-later,
// a successor to OB-Xd, and Honeydew's glue) and the 493 factory patches OB-Xf ships (`banks/`). The editor and the
// settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::instrument::{Instrument, Synth};
use ml_small_fx::{Mark, Spec, editor::Extension, params::FxParams};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::{
    os::raw::{c_char, c_int, c_void},
    sync::Arc,
};

pub mod import;
pub mod presets;
pub mod settings;
mod ui;

use settings::{COUNT, KINDS, SETTINGS, XML};

pub type Effect = Instrument<Ob>;

unsafe extern "C" {
    fn obx_new(rate: f64) -> *mut c_void;
    fn obx_free(o: *mut c_void);
    fn obx_param(o: *mut c_void, id: *const c_char, value: f32) -> c_int;
    fn obx_note_on(o: *mut c_void, note: c_int, velocity: f32, channel: c_int);
    fn obx_note_off(o: *mut c_void, note: c_int, channel: c_int);
    fn obx_pitch_bend(o: *mut c_void, value: f32);
    fn obx_mod_wheel(o: *mut c_void, value: f32);
    fn obx_sustain(o: *mut c_void, on: c_int);
    fn obx_all_off(o: *mut c_void);
    fn obx_render(o: *mut c_void, left: *mut f32, right: *mut f32, frames: c_int);
}

/// The saved state's tail: the patch that was loaded (its place in the list, 65535 when it is none or was edited).
const TAIL: [u8; 2] = [255, 255];

pub static SPEC: Spec = Spec {
    name: "Rind OB",
    word: "OB",
    about: "A two-oscillator analogue-style polysynth: the OB-Xf engine and its 493 factory patches.",
    settings: &SETTINGS,
    studio_settings: 0,
    version: None,
    any_version: false,
    fields: &[],
    tail: &TAIL,
    size: (760, 560),
    cards: &[],
    controls: &[],
    display: None,
    mark: Mark { x: 744., y: 548., anchor: 1., stacked: false },
    hint: None,
    labels: &[],
    text,
    parse,
    sound: |_, _| unreachable!(),
    rings: true,
};

/// OB-Xf's own scales (`ParamScales.h`).
fn logsc(n: f64, min: f64, max: f64, rolloff: f64) -> f64 {
    (((n * (rolloff + 1.).ln()).exp() - 1.) / rolloff) * (max - min) + min
}

fn time_text(ms: f64) -> String {
    if ms >= 1000. { format!("{:.2} s", ms / 1000.) } else { format!("{ms:.0} ms") }
}

pub const LEGATO: [&str; 4] = ["Both", "Filter", "Amp", "Retrigger"];
pub const PRIORITY: [&str; 3] = ["Last", "Low", "High"];
pub const NOISE: [&str; 3] = ["White", "Pink", "Red"];
pub const LFO_TO: [&str; 3] = ["Off", "On", "Inv"];

/// The voices a stored value means (polyphony, unison voices).
pub fn voices(v: i32) -> i32 {
    1 + ((f64::from(v) / 1000. * 32.) as i32).clamp(0, 31)
}
/// The stored value for `n` voices.
pub fn voices_value(n: i32) -> i32 {
    (((f64::from(n.clamp(1, 32)) - 0.5) / 32.) * 1000.).round() as i32
}
pub fn legato(v: i32) -> usize {
    ((f64::from(v) / 1000. * 3.) as usize).min(3)
}
pub fn priority(v: i32) -> usize {
    ((f64::from(v) / 1000. * 2.) as usize).min(2)
}
pub fn noise(v: i32) -> usize {
    if v < 333 { 0 } else if v < 667 { 1 } else { 2 }
}
pub fn xpander(v: i32) -> i32 {
    (f64::from(v) / 1000. * 14.).round() as i32 + 1
}
/// 0 off, 1 on, 2 inverted.
pub fn tri(v: i32) -> usize {
    ((f64::from(v) / 500.).round() as usize).min(2)
}

fn text(k: usize, v: i32) -> String {
    use settings::*;
    let n = f64::from(v) / 1000.;
    match KINDS.get(k).copied().unwrap_or(b'f') {
        b'b' if k == VIBRATO_WAVE => if v >= 500 { "Square" } else { "Sine" }.to_string(),
        b'b' => if v >= 500 { "On" } else { "Off" }.to_string(),
        b't' => LFO_TO[tri(v)].to_string(),
        b'v' => format!("{}", voices(v)),
        b'l' => LEGATO[legato(v)].to_string(),
        b'p' => PRIORITY[priority(v)].to_string(),
        b'x' => format!("{}", xpander(v)),
        b'n' => NOISE[noise(v)].to_string(),
        _ => match k {
            TRANSPOSE => format!("{:+}", ((n * 2. - 1.) * 24.).round() as i32),
            TUNE => format!("{:+.1} ct", (n * 2. - 1.) * 100.),
            OSC1_PITCH | OSC2_PITCH => format!("{:+.2}", n * 48. - 24.),
            PITCH_BEND_UP | PITCH_BEND_DOWN => format!("{:.1} st", n * 48.),
            ENV_TO_PITCH_AMOUNT => format!("{:.1} st", n * 36.),
            VIBRATO_RATE => format!("{:.2} Hz", 2. + 10. * n),
            OSC2_DETUNE => format!("{:.1} ct", logsc(n, 0.001, 0.6, 19.) * 100.),
            FILTER_CUTOFF => {
                let hz = 440. * ((n * 120. - 45.) / 12.).exp2();
                if hz >= 1000. { format!("{:.2} kHz", hz / 1000.) } else { format!("{hz:.0} Hz") }
            }
            LFO1_RATE | LFO2_RATE => format!("{:.2} Hz", logsc(n, 0., 250., 3775.)),
            FILTER_ENV_ATTACK | FILTER_ENV_DECAY | FILTER_ENV_RELEASE => time_text(logsc(n, 1., 60000., 900.)),
            AMP_ENV_ATTACK | AMP_ENV_DECAY => time_text(logsc(n, 4., 60000., 900.)),
            AMP_ENV_RELEASE => time_text(logsc(n, 8., 60000., 900.)),
            LFO1_WAVE1 | LFO1_WAVE2 | LFO1_WAVE3 | LFO2_WAVE1 | LFO2_WAVE2 | LFO2_WAVE3 => {
                format!("{:+}%", ((n * 2. - 1.) * 100.).round() as i32)
            }
            PAN_VOICE1..=PAN_VOICE8 => {
                let p = ((n * 2. - 1.) * 100.).round() as i32;
                match p {
                    0 => "C".to_string(),
                    p if p < 0 => format!("{} L", -p),
                    p => format!("{p} R"),
                }
            }
            _ => format!("{}%", (n * 100.).round() as i32),
        },
    }
}

fn parse(k: usize, t: &str) -> Option<i32> {
    let s = SETTINGS.get(k)?;
    let word = t.split_whitespace().next()?;
    match KINDS[k] {
        b'b' => match word.to_ascii_lowercase().as_str() {
            "on" | "1" | "square" => Some(1000),
            "off" | "0" | "sine" => Some(0),
            _ => None,
        },
        b'v' => Some(voices_value(word.parse::<i32>().ok()?)),
        _ => {
            let n = word.trim_end_matches('%').parse::<f64>().ok().filter(|n| n.is_finite())?;
            Some(((n * 10.).round() as i32).clamp(s.range.0, s.range.1))
        }
    }
}

struct Engine(*mut c_void);
// The engine is one voice pool, used from the audio thread only.
unsafe impl Send for Engine {}
impl Drop for Engine {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { obx_free(self.0) }
        }
    }
}

pub struct Ob {
    engine: Engine,
    /// The settings as last sent to the engine.
    sent: Option<[i32; COUNT]>,
}
impl Default for Ob {
    fn default() -> Self {
        Self { engine: Engine(std::ptr::null_mut()), sent: None }
    }
}
impl Ob {
    /// The settings that changed into the engine.
    fn apply(&mut self, p: &FxParams) {
        if self.engine.0.is_null() {
            return;
        }
        let now: [i32; COUNT] = std::array::from_fn(|k| p.v(k));
        for k in 0..COUNT {
            if self.sent.is_none_or(|s| s[k] != now[k]) {
                unsafe { obx_param(self.engine.0, XML[k].as_ptr().cast(), now[k] as f32 / 1000.) };
            }
        }
        self.sent = Some(now);
    }
}
impl Synth for Ob {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-ob";
    const CLASS: [u8; 16] = *b"RindOB0000000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.engine = Engine(unsafe { obx_new(f64::from(rate)) });
        self.sent = None;
        self.apply(p);
    }
    fn reset(&mut self, _: &FxParams) {
        if !self.engine.0.is_null() {
            unsafe { obx_all_off(self.engine.0) };
        }
    }
    fn note_on(&mut self, _: u8, note: u8, velocity: f32, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        unsafe { obx_note_on(self.engine.0, c_int::from(note), velocity.clamp(0.01, 1.), 0) };
    }
    fn note_off(&mut self, _: u8, note: u8, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        unsafe { obx_note_off(self.engine.0, c_int::from(note), 0) };
    }
    fn pitch_bend(&mut self, value: f32) {
        if !self.engine.0.is_null() {
            unsafe { obx_pitch_bend(self.engine.0, value.clamp(0., 1.) * 2. - 1.) };
        }
    }
    fn control(&mut self, cc: u8, value: f32) {
        if self.engine.0.is_null() {
            return;
        }
        let v = value.clamp(0., 1.);
        unsafe {
            match cc {
                1 => obx_mod_wheel(self.engine.0, v),
                64 => obx_sustain(self.engine.0, c_int::from(v >= 0.5)),
                120 | 123 => obx_all_off(self.engine.0),
                _ => {}
            }
        }
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        self.apply(p);
        if self.engine.0.is_null() {
            l.fill(0.);
            r.fill(0.);
            return;
        }
        let n = l.len().min(r.len());
        let mut at = 0;
        while at < n {
            let len = (n - at).min(256);
            unsafe { obx_render(self.engine.0, l[at..].as_mut_ptr(), r[at..].as_mut_ptr(), len as c_int) };
            at += len;
        }
        for x in l[..n].iter_mut().chain(&mut r[..n]) {
            if !x.is_finite() {
                *x = 0.;
            }
        }
    }
    fn panel(p: Arc<FxParams>) -> Arc<dyn Extension> {
        ui::Panel::new(p)
    }
}

#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);
