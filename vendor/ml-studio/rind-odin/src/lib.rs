// SPDX-License-Identifier: GPL-3.0-or-later
// Rind Odin: a three-oscillator hybrid polysynth with Odin 2's engine (`cpp/`: Thomas Hezel's Odin 2, GPL-3.0-or-later,
// built against a small stand-in for the parts of JUCE it uses, and Honeydew's glue), the 260 factory patches Odin 2
// ships (`banks/`) and any `.odin` patch other people made. The editor and the settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::instrument::{Instrument, Synth};
use ml_small_fx::{Mark, Spec, editor::Extension, params::{self, FxParams}};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::{
    ffi::CString,
    os::raw::{c_char, c_int, c_void},
    sync::{
        Arc, OnceLock,
        atomic::Ordering,
        mpsc::{Receiver, Sender, channel},
    },
};

pub mod patch;
pub mod presets;
pub mod settings;
mod ui;

use settings::{COUNT, INFO, Kind, PICK, SETTINGS};

pub type Effect = Instrument<Odin>;

unsafe extern "C" {
    fn odin_new(rate: f32) -> *mut c_void;
    fn odin_free(o: *mut c_void);
    fn odin_load(o: *mut c_void, data: *const u8, size: usize) -> c_int;
    fn odin_event(o: *mut c_void, status: c_int, d1: c_int, d2: c_int, at: c_int);
    fn odin_render(o: *mut c_void, l: *mut f32, r: *mut f32, n: c_int);
    fn odin_set(o: *mut c_void, id: *const c_char, value: f32) -> c_int;
    fn odin_set_prop(o: *mut c_void, tree: *const c_char, key: *const c_char, value: f64, as_int: c_int) -> c_int;
    fn odin_tempo(o: *mut c_void, bpm: f32);
}

/// The saved state's tail: the place of the factory patch that was loaded (65535 when none), then, when none, the file of
/// the patch that was loaded (empty: the init patch). Edits to the settings come on top of either.
const TAIL: [u8; 2] = [255, 255];
/// The place a factory patch has in the saved state when none is loaded.
pub const NONE: u16 = 0xFFFF;

pub static SPEC: Spec = Spec {
    name: "Rind Odin",
    word: "ODIN",
    about: "A three-oscillator hybrid polysynth: the Odin 2 engine, its 260 factory patches and any .odin patch.",
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

/// A setting as the hint and the host show it.
pub fn text(k: usize, v: i32) -> String {
    let Some(i) = INFO.get(k) else { return String::new() };
    let x = patch::real(k, v);
    match i.unit {
        b'n' => {
            let name = PICK[usize::from(i.table)].get(v.max(0) as usize).map_or("", |t| t.1);
            name.to_string()
        }
        b'b' => if v >= 1 { "On" } else { "Off" }.to_string(),
        b'h' if x >= 1000. => format!("{:.2} kHz", x / 1000.),
        b'h' if x >= 100. => format!("{x:.0} Hz"),
        b'h' => format!("{x:.2} Hz"),
        b'd' => format!("{x:+.1} dB"),
        b's' if x < 1. => format!("{:.0} ms", x * 1000.),
        b's' => format!("{x:.2} s"),
        b'c' => format!("{x:+.0} ct"),
        b't' => format!("{v:+} st"),
        b'o' => format!("{v:+} oct"),
        b'%' => format!("{:.0}%", x * 100.),
        b'x' => format!("{:+.0}%", x * 100.),
        b'u' => format!("{}", v + 1),
        b'i' => format!("{v}"),
        _ => format!("{x:.2}"),
    }
}

fn parse(k: usize, t: &str) -> Option<i32> {
    let i = INFO.get(k)?;
    let s = SETTINGS.get(k)?;
    let word = t.split_whitespace().next()?;
    match i.unit {
        b'n' => {
            let w = word.to_ascii_lowercase();
            PICK[usize::from(i.table)].iter().position(|t| t.1.to_ascii_lowercase().starts_with(&w)).map(|p| p as i32)
        }
        b'b' => match word.to_ascii_lowercase().as_str() {
            "on" | "1" => Some(1),
            "off" | "0" => Some(0),
            _ => None,
        },
        unit => {
            let mut n = word.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%').parse::<f64>().ok().filter(|n| n.is_finite())?;
            let lower = t.to_ascii_lowercase();
            n = match unit {
                b'%' | b'x' => n / 100.,
                b'h' if lower.contains('k') => n * 1000.,
                b's' if lower.contains("ms") => n / 1000.,
                b'u' => n - 1.,
                _ => n,
            };
            Some(patch::setting_of(k, n).clamp(s.range.0, s.range.1))
        }
    }
}

/// The engine's names for the settings, NUL-ended (a parameter's id, or a property's tree and key).
fn names() -> &'static [(CString, CString)] {
    static N: OnceLock<Vec<(CString, CString)>> = OnceLock::new();
    N.get_or_init(|| {
        INFO.iter()
            .map(|i| (CString::new(i.tree).unwrap_or_default(), CString::new(i.key).unwrap_or_default()))
            .collect()
    })
}

struct Engine(*mut c_void);
// An engine is one voice pool, used from one thread at a time: the audio thread's, or the loader's while it is built.
unsafe impl Send for Engine {}
impl Drop for Engine {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { odin_free(self.0) }
        }
    }
}

/// Where the patch the sound loads comes from.
#[derive(Clone)]
pub(crate) enum Source {
    Init,
    Factory(usize),
    File(Vec<u8>),
}

/// What the saved state says is loaded (its tail, after the settings).
pub(crate) fn source_of(raw: &[u8]) -> Source {
    let at = params::state_len(&SPEC);
    let Some(t) = raw.get(at..at + 2) else { return Source::Init };
    let i = u16::from_le_bytes([t[0], t[1]]);
    if i != NONE && usize::from(i) < presets::bank().patches.len() {
        return Source::Factory(usize::from(i));
    }
    match raw.get(at + 2..) {
        Some(file) if !file.is_empty() => Source::File(file.to_vec()),
        _ => Source::Init,
    }
}

/// A cheap name for a source, to tell whether the patch changed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key(u8, usize, u64);
impl Source {
    pub(crate) fn key(&self) -> Key {
        match self {
            Source::Init => Key(0, 0, 0),
            Source::Factory(i) => Key(1, *i, 0),
            Source::File(b) => Key(2, b.len(), b.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &c| (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3))),
        }
    }
}

/// An engine with the patch loaded, and the settings the patch means.
fn build(rate: f32, source: &Source) -> (Engine, Vec<i32>) {
    let engine = Engine(unsafe { odin_new(rate) });
    let defaults = || SETTINGS.iter().map(|s| s.start).collect::<Vec<_>>();
    if engine.0.is_null() {
        return (engine, defaults());
    }
    let data: &[u8] = match source {
        Source::Init => &[],
        Source::Factory(i) => presets::bank().patches.get(*i).map_or(&[][..], |p| p.data),
        Source::File(b) => b,
    };
    if data.is_empty() {
        return (engine, defaults());
    }
    // The engine reads the file itself; this reads the settings it means, which must agree on what a patch is.
    let values = match patch::parse(data) {
        Ok(p) if unsafe { odin_load(engine.0, data.as_ptr(), data.len()) } != 0 => p.values,
        _ => return (Engine(unsafe { odin_new(rate) }), defaults()),
    };
    (engine, values)
}

enum Job {
    Build { rate: f32, source: Source, serial: u64 },
    /// An engine the sound is done with, freed here so that the audio thread does not.
    Free(Engine),
}
struct Built {
    engine: Engine,
    values: Vec<i32>,
    serial: u64,
}

/// Builds engines on a thread of its own: a patch takes some 0.4 s to build, far too long for the audio thread.
fn worker(rx: Receiver<Job>, tx: Sender<Built>) {
    let mut queue: Vec<Job> = Vec::new();
    while let Ok(job) = rx.recv() {
        queue.push(job);
        while let Ok(more) = rx.try_recv() {
            queue.push(more);
        }
        // Only the newest patch matters: engines for the ones before it are not built.
        let last = queue.iter().rposition(|j| matches!(j, Job::Build { .. }));
        for (n, job) in std::mem::take(&mut queue).into_iter().enumerate() {
            match job {
                Job::Build { rate, source, serial } if Some(n) == last => {
                    let (engine, values) = build(rate, &source);
                    if tx.send(Built { engine, values, serial }).is_err() {
                        return;
                    }
                }
                _ => {}
            }
        }
    }
}

struct Loader {
    jobs: Sender<Job>,
    built: Receiver<Built>,
}

pub struct Odin {
    engine: Engine,
    /// The settings as last sent to the engine (or the patch it was built with).
    sent: Vec<i32>,
    rate: f32,
    /// `raw_version` last looked at, the patch it held and the number of the engine asked for.
    seen: u64,
    key: Option<Key>,
    serial: u64,
    pending: bool,
    loader: Option<Loader>,
    tempo: f64,
    tempo_sent: f64,
}
impl Default for Odin {
    fn default() -> Self {
        Self {
            engine: Engine(std::ptr::null_mut()),
            sent: SETTINGS.iter().map(|s| s.start).collect(),
            rate: 48000.,
            seen: 0,
            key: None,
            serial: 0,
            pending: false,
            loader: None,
            tempo: 120.,
            tempo_sent: f64::NAN,
        }
    }
}
impl Odin {
    fn send(&self, k: usize, v: i32) {
        let (tree, key) = &names()[k];
        let x = patch::real(k, v);
        unsafe {
            match INFO[k].kind {
                Kind::Float | Kind::Int => odin_set(self.engine.0, key.as_ptr(), x as f32),
                Kind::Pick | Kind::Prop => odin_set_prop(self.engine.0, tree.as_ptr(), key.as_ptr(), x, 1),
                Kind::Amount => odin_set_prop(self.engine.0, tree.as_ptr(), key.as_ptr(), x, 0),
            };
        }
    }

    /// The settings that changed into the engine.
    fn apply(&mut self, p: &FxParams) {
        if self.engine.0.is_null() || self.pending {
            return;
        }
        for k in 0..COUNT {
            let now = p.v(k);
            if self.sent[k] != now {
                self.send(k, now);
                self.sent[k] = now;
            }
        }
        if self.tempo != self.tempo_sent {
            unsafe { odin_tempo(self.engine.0, self.tempo as f32) };
            self.tempo_sent = self.tempo;
        }
    }

    /// Asks for a new engine when the saved state names another patch, and takes the one that was built.
    fn follow(&mut self, p: &FxParams) {
        let Some(loader) = &self.loader else { return };
        let version = p.raw_version.load(Ordering::Acquire);
        if version != self.seen {
            self.seen = version;
            let source = source_of(&p.raw_copy());
            if self.key != Some(source.key()) {
                self.key = Some(source.key());
                self.serial += 1;
                self.pending = true;
                let _ = loader.jobs.send(Job::Build { rate: self.rate, source, serial: self.serial });
            }
        }
        while let Ok(b) = loader.built.try_recv() {
            if b.serial == self.serial {
                let old = std::mem::replace(&mut self.engine, b.engine);
                let _ = loader.jobs.send(Job::Free(old));
                self.sent = b.values;
                self.pending = false;
                self.tempo_sent = f64::NAN;
            } else {
                let _ = loader.jobs.send(Job::Free(b.engine));
            }
        }
    }

    fn event(&self, status: u8, d1: u8, d2: u8) {
        if !self.engine.0.is_null() {
            unsafe { odin_event(self.engine.0, c_int::from(status), c_int::from(d1), c_int::from(d2), 0) };
        }
    }
}
impl Synth for Odin {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-odin";
    const CLASS: [u8; 16] = *b"RindOdin00000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate;
        // The first engine is built here, where waiting is fine, so that the sound is there from the first block (a
        // bounce starts at once).
        let source = source_of(&p.raw_copy());
        let (engine, values) = build(rate, &source);
        self.engine = engine;
        self.sent = values;
        self.key = Some(source.key());
        self.seen = p.raw_version.load(Ordering::Acquire);
        self.pending = false;
        self.tempo_sent = f64::NAN;
        let (jobs, rx) = channel();
        let (tx, built) = channel();
        self.serial = 0;
        if std::thread::Builder::new().name("rind-odin-loader".into()).spawn(move || worker(rx, tx)).is_ok() {
            self.loader = Some(Loader { jobs, built });
        } else {
            self.loader = None;
        }
        self.apply(p);
    }
    fn reset(&mut self, _: &FxParams) {
        self.event(0xB0, 123, 0);
        self.event(0xB0, 120, 0);
    }
    fn note_on(&mut self, channel: u8, note: u8, velocity: f32, _: Option<i32>) {
        if note > 127 {
            return;
        }
        let v = (velocity.clamp(0., 1.) * 127.).round().max(1.) as u8;
        self.event(0x90 | (channel & 15), note, v);
    }
    fn note_off(&mut self, channel: u8, note: u8, _: Option<i32>) {
        if note > 127 {
            return;
        }
        self.event(0x80 | (channel & 15), note, 0);
    }
    fn pitch_bend(&mut self, value: f32) {
        let v = (value.clamp(0., 1.) * 16383.).round() as u16;
        self.event(0xE0, (v & 0x7f) as u8, (v >> 7) as u8);
    }
    fn control(&mut self, cc: u8, value: f32) {
        if cc < 128 {
            self.event(0xB0, cc, (value.clamp(0., 1.) * 127.).round() as u8);
        }
    }
    fn pressure(&mut self, value: f32) {
        self.event(0xD0, (value.clamp(0., 1.) * 127.).round() as u8, 0);
    }
    fn tempo(&mut self, bpm: f64) {
        if bpm.is_finite() && (20. ..=400.).contains(&bpm) {
            self.tempo = bpm;
        }
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        self.follow(p);
        self.apply(p);
        let n = l.len().min(r.len());
        if self.engine.0.is_null() {
            l.fill(0.);
            r.fill(0.);
            return;
        }
        let mut at = 0;
        while at < n {
            let len = (n - at).min(512);
            unsafe { odin_render(self.engine.0, l[at..].as_mut_ptr(), r[at..].as_mut_ptr(), len as c_int) };
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
