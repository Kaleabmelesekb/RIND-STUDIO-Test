// SPDX-License-Identifier: GPL-3.0-or-later
// Rind FM: a six-operator FM synthesizer with the whole DX7 voice to edit, and Dexed's engine: its three engines
// (modern, Mark I, OPL), filter, mono mode, portamento and glissando, pitch wheel and the four controllers (`cpp/`:
// Dexed's msfa sources, GPL-3.0-or-later, and Honeydew's glue; msfa itself is Google's, Apache-2.0), with the voice
// banks Dexed ships (`banks/`). The editor and these settings are Honeydew's.
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
    fn rdx_set_globals(r: *mut c_void, globals: *const i32);
    fn rdx_cc(r: *mut c_void, cc: c_int, value: c_int);
    fn rdx_pressure(r: *mut c_void, value: c_int);
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
/// Dexed's settings that are not part of a voice.
pub const MONO: usize = 152;
pub const ENGINE: usize = 153;
pub const CUTOFF: usize = 154;
pub const RESO: usize = 155;
pub const TUNE: usize = 156;
pub const BEND_UP: usize = 157;
pub const BEND_DOWN: usize = 158;
pub const BEND_STEP: usize = 159;
pub const PORTA: usize = 160;
pub const GLISS: usize = 161;
/// The four controllers (mod wheel, foot, breath, aftertouch) have four settings each: range, pitch, amp, EG bias.
pub const CONTROLLERS: usize = 162;

/// Setting `i` (0 range, 1 pitch, 2 amp, 3 EG bias) of controller `c` (0 wheel, 1 foot, 2 breath, 3 aftertouch).
pub const fn ctrl_k(c: usize, i: usize) -> usize {
    CONTROLLERS + 4 * c + i
}

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
const ENGINES: [&str; 3] = ["Modern", "Mark I", "OPL"];
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
        MONO | GLISS => on_off(v),
        ENGINE => ENGINES[v.clamp(0, 2) as usize].to_string(),
        CUTOFF | RESO => format!("{}%", v / 10),
        TUNE => format!("{:+.1} ct", f64::from(v) / 10.),
        k if k >= CONTROLLERS && (k - CONTROLLERS) % 4 != 0 => on_off(v),
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
    } else if k == ENGINE {
        find(&ENGINES)
    } else {
        None
    };
    if let Some(v) = v {
        return Some(v);
    }
    let flag = k >= CONTROLLERS && (k - CONTROLLERS) % 4 != 0;
    if k >= OP_ON && k < OUTPUT || matches!(k, OSC_SYNC | LFO_SYNC | MONO | GLISS) || flag {
        return match word.to_ascii_lowercase().as_str() {
            "on" | "1" => Some(1),
            "off" | "0" => Some(0),
            _ => None,
        };
    }
    let n = word.parse::<f64>().ok().filter(|n| n.is_finite())?;
    let raw = match k {
        ALGORITHM => n - 1.,
        OUTPUT | TUNE => n * 10.,
        CUTOFF | RESO => n * 10.,
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
    /// The settings that are not part of the voice, as `rdx_set_globals` takes them, as last sent.
    sent: Option<[i32; 19]>,
    gain: f32,
}
impl Default for Fm {
    fn default() -> Self {
        Self { engine: Engine::new(), made: None, sent: None, gain: 1. }
    }
}
impl Fm {
    /// The settings into the engine: a changed setting makes the patch the notes play, sounding ones included.
    fn apply(&mut self, p: &FxParams) {
        if self.engine.0.is_null() {
            return;
        }
        let now: [i32; COUNT] = std::array::from_fn(|k| p.v(k));
        self.gain = 10f32.powf(now[OUTPUT] as f32 / 200.);
        let globals = [
            now[MONO],
            now[ENGINE],
            now[CUTOFF],
            now[RESO],
            1000,
            now[TUNE],
            now[BEND_UP],
            now[BEND_DOWN],
            now[BEND_STEP],
            now[PORTA],
            now[GLISS],
            now[ctrl_k(0, 0)],
            now[ctrl_k(1, 0)],
            now[ctrl_k(2, 0)],
            now[ctrl_k(3, 0)],
            0,
            0,
            0,
            0,
        ];
        let mut globals = globals;
        for c in 0..4 {
            globals[15 + c] = now[ctrl_k(c, 1)] | now[ctrl_k(c, 2)] << 1 | now[ctrl_k(c, 3)] << 2;
        }
        if self.sent != Some(globals) {
            self.sent = Some(globals);
            unsafe { rdx_set_globals(self.engine.0, globals.as_ptr()) };
        }
        let mut key = now;
        key[OUTPUT] = 0;
        for k in MONO..COUNT {
            key[k] = 0;
        }
        if self.made == Some(key) {
            return;
        }
        self.made = Some(key);
        let mut patch = [0 as c_char; 156];
        for k in 0..VOICE_BYTES {
            patch[k] = now[k].clamp(0, 127) as c_char;
        }
        // The operator switches, bit 5 for operator 1.
        let mut on = 0;
        for n in 1..=6 {
            if now[OP_ON + n - 1] != 0 {
                on |= 1 << (6 - n);
            }
        }
        patch[155] = on;
        unsafe { rdx_set_patch(self.engine.0, patch.as_ptr()) };
    }
}
impl Synth for Fm {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-fm";
    const CLASS: [u8; 16] = *b"RindFM0000000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        unsafe { rdx_init(f64::from(rate)) };
        self.made = None;
        self.sent = None;
        if !self.engine.0.is_null() {
            unsafe { rdx_all_off(self.engine.0) };
        }
        self.apply(p);
    }
    fn reset(&mut self, _: &FxParams) {
        if !self.engine.0.is_null() {
            unsafe { rdx_all_off(self.engine.0) };
        }
    }
    /// The engine shifts the keys by the voice's transpose itself, as Dexed does, and stands the notes down by the
    /// key that was played.
    fn note_on(&mut self, _: u8, note: u8, velocity: f32, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        let velocity = (velocity.clamp(0., 1.) * 127.).round().max(1.) as c_int;
        unsafe { rdx_note_on(self.engine.0, c_int::from(note), velocity) };
    }
    fn note_off(&mut self, _: u8, note: u8, _: Option<i32>) {
        if self.engine.0.is_null() || note > 127 {
            return;
        }
        unsafe { rdx_note_off(self.engine.0, c_int::from(note)) };
    }
    fn pitch_bend(&mut self, value: f32) {
        if !self.engine.0.is_null() {
            let v = (value.clamp(0., 1.) * 16383.).round() as c_int;
            unsafe { rdx_pitch_bend(self.engine.0, v) };
        }
    }
    fn control(&mut self, cc: u8, value: f32) {
        if !self.engine.0.is_null() {
            let v = (value.clamp(0., 1.) * 127.).round() as c_int;
            unsafe { rdx_cc(self.engine.0, c_int::from(cc), v) };
        }
    }
    fn pressure(&mut self, value: f32) {
        if !self.engine.0.is_null() {
            let v = (value.clamp(0., 1.) * 127.).round() as c_int;
            unsafe { rdx_pressure(self.engine.0, v) };
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
