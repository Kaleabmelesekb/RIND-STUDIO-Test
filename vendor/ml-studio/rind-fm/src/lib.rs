// SPDX-License-Identifier: GPL-3.0-or-later
// Rind FM: a six-operator FM synthesizer. The sound engine is msfa's (Google's music-synthesizer-for-android, the
// engine Dexed grew from; Apache-2.0, `cpp/`), with the voice banks Dexed ships (`banks/`). The editor and these
// settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::{extra::Notes, instrument::Instrument, instrument::Synth};
use ml_small_fx::{
    Card, Control, Display, Mark, Setting, Spec, editor::Extension, params::FxParams,
};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use os_ui::{Canvas, Color, Font, Rect};
use std::{
    os::raw::{c_char, c_int, c_void},
    sync::Arc,
};

pub type Effect = Instrument<Fm>;

unsafe extern "C" {
    fn rdx_init(rate: f64);
    fn rdx_new() -> *mut c_void;
    fn rdx_free(r: *mut c_void);
    fn rdx_unpack(packed: *const c_char, unpacked: *mut c_char);
    fn rdx_set_patch(r: *mut c_void, unpacked: *const c_char);
    fn rdx_note_on(r: *mut c_void, note: c_int, velocity: c_int);
    fn rdx_note_off(r: *mut c_void, note: c_int);
    fn rdx_pitch_bend(r: *mut c_void, value: c_int);
    fn rdx_all_off(r: *mut c_void);
    fn rdx_render(r: *mut c_void, out: *mut f32, frames: c_int);
}

/// The voices of every bank, 32 of 128 bytes each (sysex banks without their frame).
static BANKS: &[u8] = include_bytes!("../banks/banks.bin");
static BANK_NAMES: &str = include_str!("../banks/names.txt");
const BANK_LEN: usize = 32 * 128;

fn banks() -> usize {
    BANKS.len() / BANK_LEN
}
fn bank_name(bank: usize) -> &'static str {
    BANK_NAMES.lines().nth(bank).unwrap_or("")
}
/// The 128 bytes of voice `voice` (0..32) of bank `bank` (0-based).
fn voice(bank: usize, voice: usize) -> &'static [u8] {
    let at = bank.min(banks() - 1) * BANK_LEN + voice.min(31) * 128;
    &BANKS[at..at + 128]
}
fn voice_name(bank: usize, v: usize) -> String {
    let raw = &voice(bank, v)[118..128];
    let name: String = raw
        .iter()
        .map(|&b| if (32..127).contains(&b) { b as char } else { ' ' })
        .collect();
    name.trim().to_string()
}

const fn s(id: &'static str, name: &'static str, range: (i32, i32), start: i32) -> Setting {
    Setting {
        id,
        name,
        range,
        start,
        magnet: None,
    }
}
const fn card(row: usize, col: usize, heading: &'static str) -> Card {
    Card {
        x0: 8. + 124. * col as f32,
        x1: 124. + 124. * col as f32,
        y0: 108. + 137. * row as f32,
        y1: 233. + 137. * row as f32,
        heading,
    }
}
const fn knob(row: usize, col: usize, id: &'static str, bipolar: bool) -> Control {
    Control::Knob {
        id,
        cx: 66. + 124. * col as f32,
        cy: 165. + 137. * row as f32,
        r: 24.,
        caption: "",
        bipolar,
    }
}

/// Setting indices.
const BANK: usize = 0;
const PATCH: usize = 1;
const VOLUME: usize = 2;
const TRANSPOSE: usize = 3;
const ALGORITHM: usize = 4;
const FEEDBACK: usize = 5;
/// `OP1` .. `OP6` levels are 6 .. 11.
const OP_LEVEL: usize = 6;
const LFO_SPEED: usize = 12;
const LFO_PITCH: usize = 13;
const LFO_WAVE: usize = 14;
const COUNT: usize = 15;

pub static SPEC: Spec = Spec {
    name: "Rind FM",
    word: "FM",
    about: "A six-operator FM synthesizer: the msfa engine with 1056 voices, in the ML Studio look.",
    settings: &[
        s("bank", "Bank", (1, 33), 1),
        s("patch", "Voice", (1, 32), 1),
        Setting {
            id: "output",
            name: "Output",
            range: (-240, 120),
            start: 0,
            magnet: Some(0),
        },
        s("transpose", "Transpose", (-24, 24), 0),
        s("algorithm", "Algorithm", (0, 32), 0),
        s("feedback", "Feedback", (0, 8), 0),
        s("op1", "Op 1 Level", (0, 100), 0),
        s("op2", "Op 2 Level", (0, 100), 0),
        s("op3", "Op 3 Level", (0, 100), 0),
        s("op4", "Op 4 Level", (0, 100), 0),
        s("op5", "Op 5 Level", (0, 100), 0),
        s("op6", "Op 6 Level", (0, 100), 0),
        s("lfo_speed", "LFO Speed", (0, 100), 0),
        s("lfo_pitch", "LFO Pitch", (0, 100), 0),
        s("lfo_wave", "LFO Wave", (0, 6), 0),
    ],
    studio_settings: COUNT,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 530),
    cards: &[
        card(0, 0, "BANK"),
        card(0, 1, "VOICE"),
        card(0, 2, "OUTPUT"),
        card(0, 3, "TRANSPOSE"),
        card(0, 4, "ALGORITHM"),
        card(0, 5, "FEEDBACK"),
        card(1, 0, "OP 1"),
        card(1, 1, "OP 2"),
        card(1, 2, "OP 3"),
        card(1, 3, "OP 4"),
        card(1, 4, "OP 5"),
        card(1, 5, "OP 6"),
        card(2, 0, "LFO SPEED"),
        card(2, 1, "LFO PITCH"),
        card(2, 2, "LFO WAVE"),
    ],
    controls: &[
        knob(0, 0, "bank", false),
        knob(0, 1, "patch", false),
        knob(0, 2, "output", true),
        knob(0, 3, "transpose", true),
        knob(0, 4, "algorithm", false),
        knob(0, 5, "feedback", false),
        knob(1, 0, "op1", false),
        knob(1, 1, "op2", false),
        knob(1, 2, "op3", false),
        knob(1, 3, "op4", false),
        knob(1, 4, "op5", false),
        knob(1, 5, "op6", false),
        knob(2, 0, "lfo_speed", false),
        knob(2, 1, "lfo_pitch", false),
        knob(2, 2, "lfo_wave", false),
    ],
    display: Some(Display {
        r: Rect::new(8., 28., 744., 72.),
        draw: lcd,
    }),
    mark: Mark {
        x: 744.,
        y: 512.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text,
    parse,
    sound: |_, _| unreachable!(),
    rings: true,
};

/// The voice's name on a dark readout, as a DX7's display shows it.
fn lcd(c: &mut Canvas, r: Rect, v: &[i32]) {
    let ink = Color::rgb(0x16191b);
    let glow = Color::rgb(0xb7e36a);
    let dim = Color::rgb(0x7f9a54);
    c.fill_rrect(r, 6., ink);
    let bank = (v[BANK] - 1).max(0) as usize;
    let at = (v[PATCH] - 1).max(0) as usize;
    let name = format!("{:02}  {}", at + 1, voice_name(bank, at));
    c.text_center(Font::new(28), Rect::new(r.x, r.y + 6., r.w, 38.), glow, &name);
    let sub = format!("BANK {:02}  {}", bank + 1, bank_name(bank).to_uppercase());
    c.text_center(Font::new(11), Rect::new(r.x, r.y + 46., r.w, 18.), dim, &sub);
}

/// An override setting: 0 leaves the voice as it is, `n` sets `n - 1`.
fn over(v: i32) -> Option<u8> {
    (v > 0).then(|| (v - 1) as u8)
}
fn patch_or(v: i32) -> String {
    match over(v) {
        Some(n) => n.to_string(),
        None => "Voice".to_string(),
    }
}
const WAVES: [&str; 6] = ["Triangle", "Saw down", "Saw up", "Square", "Sine", "Sample+hold"];
fn text(k: usize, v: i32) -> String {
    match k {
        BANK => format!("{v}"),
        PATCH => format!("{v}"),
        VOLUME => format!("{:+.1} dB", v as f64 / 10.),
        TRANSPOSE => format!("{v:+} st"),
        ALGORITHM => match v {
            0 => "Voice".to_string(),
            n => format!("{n}"),
        },
        FEEDBACK | LFO_SPEED | LFO_PITCH | 6..=11 => patch_or(v),
        LFO_WAVE => match over(v) {
            Some(n) => WAVES[(n as usize).min(5)].to_string(),
            None => "Voice".to_string(),
        },
        _ => String::new(),
    }
}
fn parse(k: usize, t: &str) -> Option<i32> {
    let word = t.split_whitespace().next()?;
    let p = SPEC.settings.get(k)?;
    if word.eq_ignore_ascii_case("voice") && !matches!(k, BANK | PATCH | VOLUME | TRANSPOSE) {
        return Some(0);
    }
    let v = word.parse::<f64>().ok().filter(|v| v.is_finite())?;
    let raw = match k {
        VOLUME => v * 10.,
        FEEDBACK | LFO_SPEED | LFO_PITCH | 6..=11 => v + 1.,
        LFO_WAVE => v + 1.,
        _ => v,
    };
    Some((raw.round() as i32).clamp(p.range.0, p.range.1))
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
    /// The settings the voice was last made from (output and transpose do not change it).
    made: Option<[i32; COUNT]>,
    /// The note each key sounds, transposed, plus one (0: the key is up).
    held: [u8; 128],
    transpose: i32,
    gain: f32,
}
impl Default for Fm {
    fn default() -> Self {
        Self {
            engine: Engine::new(),
            made: None,
            held: [0; 128],
            transpose: 0,
            gain: 1.,
        }
    }
}
impl Fm {
    /// The settings into the engine: a changed voice or override makes the patch the next notes play.
    fn apply(&mut self, p: &FxParams) {
        if self.engine.0.is_null() {
            return;
        }
        let now: [i32; COUNT] = std::array::from_fn(|k| p.v(k));
        self.transpose = now[TRANSPOSE];
        self.gain = 10f32.powf(now[VOLUME] as f32 / 200.);
        let mut key = now;
        key[VOLUME] = 0;
        key[TRANSPOSE] = 0;
        if self.made == Some(key) {
            return;
        }
        self.made = Some(key);
        let packed = voice((now[BANK] - 1).max(0) as usize, (now[PATCH] - 1).max(0) as usize);
        let mut patch = [0 as c_char; 156];
        let bytes: Vec<c_char> = packed.iter().map(|&b| b as c_char).collect();
        unsafe { rdx_unpack(bytes.as_ptr(), patch.as_mut_ptr()) };
        // Operator 1 is the last of the six a voice stores.
        for op in 0..6 {
            if let Some(level) = over(now[OP_LEVEL + op]) {
                patch[(5 - op) * 21 + 16] = level.min(99) as c_char;
            }
        }
        if now[ALGORITHM] > 0 {
            patch[134] = (now[ALGORITHM] - 1).clamp(0, 31) as c_char;
        }
        if let Some(n) = over(now[FEEDBACK]) {
            patch[135] = n.min(7) as c_char;
        }
        if let Some(n) = over(now[LFO_SPEED]) {
            patch[137] = n.min(99) as c_char;
        }
        if let Some(n) = over(now[LFO_PITCH]) {
            patch[139] = n.min(99) as c_char;
        }
        if let Some(n) = over(now[LFO_WAVE]) {
            patch[142] = n.min(5) as c_char;
        }
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
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[
            "The sound engine is msfa's (Google, Apache-2.0), the one Dexed grew from; the voices are Dexed's banks (GPL-3.0).",
        ]))
    }
}

#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);
