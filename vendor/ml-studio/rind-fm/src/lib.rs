// SPDX-License-Identifier: GPL-3.0-or-later
// Rind FM: a six-operator FM synthesizer with the whole DX7 voice to edit. The sound engine is msfa's (Google's
// music-synthesizer-for-android, the engine Dexed grew from; Apache-2.0, `cpp/`), with the voice banks Dexed ships
// (`banks/`). The editor and these settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::instrument::{Instrument, Synth};
use ml_small_fx::{Mark, Spec, editor::Extension, params::FxParams};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::{
    os::raw::{c_char, c_int, c_void},
    sync::Arc,
};

pub mod algo;
pub mod banks;
pub mod settings;
mod ui;

use settings::{COUNT, SETTINGS};

pub type Effect = Instrument<Fm>;

unsafe extern "C" {
    fn rdx_init(rate: f64);
    fn rdx_new() -> *mut c_void;
    fn rdx_free(r: *mut c_void);
    pub(crate) fn rdx_unpack(packed: *const c_char, unpacked: *mut c_char);
    fn rdx_set_patch(r: *mut c_void, unpacked: *const c_char);
    fn rdx_note_on(r: *mut c_void, note: c_int, velocity: c_int);
    fn rdx_note_off(r: *mut c_void, note: c_int);
    fn rdx_pitch_bend(r: *mut c_void, value: c_int);
    fn rdx_all_off(r: *mut c_void);
    fn rdx_render(r: *mut c_void, out: *mut f32, frames: c_int);
}

/// The settings are the 145 bytes of a DX7 voice in the order the engine plays them (the six operators, operator 6
/// first, 21 each; then the pitch envelope and the globals), then the six operator switches and the output.
pub const VOICE_BYTES: usize = 145;
pub const OP_R1: usize = 0;
pub const OP_L1: usize = 4;
pub const OP_BP: usize = 8;
pub const OP_LD: usize = 9;
pub const OP_RD: usize = 10;
pub const OP_LC: usize = 11;
pub const OP_RC: usize = 12;
pub const OP_RS: usize = 13;
pub const OP_AMS: usize = 14;
pub const OP_KVS: usize = 15;
pub const OP_LVL: usize = 16;
pub const OP_MODE: usize = 17;
pub const OP_COARSE: usize = 18;
pub const OP_FINE: usize = 19;
pub const OP_DET: usize = 20;
pub const PEG_R: usize = 126;
pub const PEG_L: usize = 130;
pub const ALGORITHM: usize = 134;
pub const FEEDBACK: usize = 135;
pub const OSC_SYNC: usize = 136;
pub const LFO_SPEED: usize = 137;
pub const LFO_DELAY: usize = 138;
pub const LFO_PMD: usize = 139;
pub const LFO_AMD: usize = 140;
pub const LFO_SYNC: usize = 141;
pub const LFO_WAVE: usize = 142;
pub const PITCH_MOD_SENS: usize = 143;
pub const TRANSPOSE: usize = 144;
pub const OP_ON: usize = 145;
pub const OUTPUT: usize = 151;

/// Setting `i` of operator `n` (1..=6, as a DX7 numbers them).
pub const fn op_k(n: usize, i: usize) -> usize {
    (6 - n) * 21 + i
}

/// The saved state's tail: the voice's bank and place (255: edited) and its name.
const TAIL: [u8; 12] = [255, 0, b'I', b'N', b'I', b'T', b' ', b'V', b'O', b'I', b'C', b'E'];

pub static SPEC: Spec = Spec {
    name: "Rind FM",
    word: "FM",
    about: "A six-operator FM synthesizer: the msfa engine, every parameter of a DX7 voice and 1056 voices.",
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

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const CURVES: [&str; 4] = ["-LIN", "-EXP", "+EXP", "+LIN"];
const WAVES: [&str; 6] = ["Triangle", "Saw down", "Saw up", "Square", "Sine", "Sample+hold"];

fn text(k: usize, v: i32) -> String {
    let on_off = |v: i32| if v != 0 { "On" } else { "Off" }.to_string();
    if k < OP_ON && k < PEG_R {
        return match k % 21 {
            OP_BP => {
                // Break point 0 is A-1; 39 is C3.
                let m = v + 21;
                format!("{}{}", NOTES[(m % 12) as usize], m / 12 - 2)
            }
            OP_LC | OP_RC => CURVES[v.clamp(0, 3) as usize].to_string(),
            OP_MODE => if v == 0 { "Ratio" } else { "Fixed" }.to_string(),
            OP_DET => format!("{:+}", v - 7),
            _ => format!("{v}"),
        };
    }
    match k {
        ALGORITHM => format!("{}", v + 1),
        OSC_SYNC | LFO_SYNC => on_off(v),
        LFO_WAVE => WAVES[v.clamp(0, 5) as usize].to_string(),
        TRANSPOSE => format!("{}{}", NOTES[(v % 12) as usize], v / 12 + 1),
        OP_ON..=150 => on_off(v),
        OUTPUT => format!("{:+.1} dB", f64::from(v) / 10.),
        _ => format!("{v}"),
    }
}

fn parse(k: usize, t: &str) -> Option<i32> {
    let s = SETTINGS.get(k)?;
    let word = t.split_whitespace().next()?;
    let find = |names: &[&str]| names.iter().position(|n| n.eq_ignore_ascii_case(word)).map(|i| i as i32);
    let v = if k % 21 == OP_LC || k % 21 == OP_RC {
        if k < PEG_R { find(&CURVES) } else { None }
    } else if k == LFO_WAVE {
        find(&WAVES)
    } else {
        None
    };
    if let Some(v) = v {
        return Some(v);
    }
    if k >= OP_ON && k < OUTPUT || matches!(k, OSC_SYNC | LFO_SYNC) {
        return match word.to_ascii_lowercase().as_str() {
            "on" | "1" => Some(1),
            "off" | "0" => Some(0),
            _ => None,
        };
    }
    let n = word.parse::<f64>().ok().filter(|n| n.is_finite())?;
    let raw = match k {
        ALGORITHM => n - 1.,
        OUTPUT => n * 10.,
        TRANSPOSE => n + 24.,
        _ if k < PEG_R && k % 21 == OP_DET => n + 7.,
        _ => n,
    };
    Some((raw.round() as i32).clamp(s.range.0, s.range.1))
}

struct Engine(*mut c_void);
// The engine is one voice pool, used from the audio thread only.
unsafe impl Send for Engine {}
impl Engine {
    fn new() -> Self {
        Self(unsafe { rdx_new() })
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rdx_free(self.0) }
        }
    }
}

pub struct Fm {
    engine: Engine,
    /// The settings the voice was last made from (the output does not change it).
    made: Option<[i32; COUNT]>,
    /// The note each key sounds, transposed, plus one (0: the key is up).
    held: [u8; 128],
    transpose: i32,
    gain: f32,
}
impl Default for Fm {
    fn default() -> Self {
        Self { engine: Engine::new(), made: None, held: [0; 128], transpose: 0, gain: 1. }
    }
}
impl Fm {
    /// The settings into the engine: a changed setting makes the patch the next notes play.
    fn apply(&mut self, p: &FxParams) {
        if self.engine.0.is_null() {
            return;
        }
        let now: [i32; COUNT] = std::array::from_fn(|k| p.v(k));
        self.transpose = now[TRANSPOSE] - 24;
        self.gain = 10f32.powf(now[OUTPUT] as f32 / 200.);
        let mut key = now;
        key[OUTPUT] = 0;
        if self.made == Some(key) {
            return;
        }
        self.made = Some(key);
        let mut patch = [0 as c_char; 156];
        for k in 0..VOICE_BYTES {
            patch[k] = now[k].clamp(0, 127) as c_char;
        }
        // An operator that is switched off has no level.
        for n in 1..=6 {
            if now[OP_ON + n - 1] == 0 {
                patch[op_k(n, OP_LVL)] = 0;
            }
        }
        patch[155] = 0x3f;
        unsafe { rdx_set_patch(self.engine.0, patch.as_ptr()) };
    }
}
impl Synth for Fm {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-fm";
    const CLASS: [u8; 16] = *b"RindFM0000000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        unsafe { rdx_init(f64::from(rate)) };
        self.held = [0; 128];
        self.made = None;
        if !self.engine.0.is_null() {
            unsafe { rdx_all_off(self.engine.0) };
        }
        self.apply(p);
    }
    fn reset(&mut self, _: &FxParams) {
        self.held = [0; 128];
        if !self.engine.0.is_null() {
            unsafe { rdx_all_off(self.engine.0) };
        }
    }
    fn note_on(&mut self, _: u8, note: u8, velocity: f32, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        let at = note as usize;
        if self.held[at] > 0 {
            unsafe { rdx_note_off(self.engine.0, c_int::from(self.held[at] - 1)) };
        }
        let played = (i32::from(note) + self.transpose).clamp(0, 127);
        self.held[at] = played as u8 + 1;
        let velocity = (velocity.clamp(0., 1.) * 127.).round() as c_int;
        unsafe { rdx_note_on(self.engine.0, played, velocity) };
    }
    fn note_off(&mut self, _: u8, note: u8, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        let at = note as usize;
        if self.held[at] > 0 {
            unsafe { rdx_note_off(self.engine.0, c_int::from(self.held[at] - 1)) };
            self.held[at] = 0;
        }
    }
    fn pitch_bend(&mut self, value: f32) {
        if !self.engine.0.is_null() {
            let v = (value.clamp(0., 1.) * 16383.).round() as c_int;
            unsafe { rdx_pitch_bend(self.engine.0, v) };
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
        unsafe { rdx_render(self.engine.0, l.as_mut_ptr(), n as c_int) };
        for (l, r) in l[..n].iter_mut().zip(&mut r[..n]) {
            *l *= self.gain;
            *r = *l;
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
