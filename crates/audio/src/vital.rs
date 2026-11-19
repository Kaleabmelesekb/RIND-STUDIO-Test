//! Vital, Matt Tytel's wavetable synthesizer, as a rack instrument.
//!
//! The sound engine is Vital's own C++ (`vendor/vital`, see its README), unchanged and compiled into the app by
//! `build.rs`: its oscillators and wavetables, voices and unison, filters, envelopes, LFOs, random sources, modulation
//! matrix and effects. This file is the host's side of it: the module's params (Enable, then
//! [`TABLE`](vital_table::TABLE), each one of Vital's own parameters or a modulation slot's source or destination), the
//! notes and controllers, and the rendering. Vital's window (OpenGL, JUCE) is not part of it: the studio's card stands
//! in for it. The web build has no C++ and renders silence.
//!
//! Vital is GPL-3.0-or-later, Matt Tytel's: <https://github.com/mtytel/vital>.

use crate::ProcessContext;
use crate::instrument::Instrument;
#[cfg(not(target_arch = "wasm32"))]
use std::ffi::{CStr, c_char, c_int, c_void};

pub use crate::vital_table::{DESTINATIONS, SLOTS, SOURCES, TABLE, VITAL_PARAMS};

/// The module's type id: Honeydew's own, after the Dragonfly reverbs.
pub const VITAL: u16 = 1012;

/// How many params the module has: Enable, then [`TABLE`].
pub const PARAMS: usize = TABLE.len() + 1;

/// The wavetables an oscillator can be given, by the name the engine reports (built by the bridge from Vital's own
/// wavetable classes): the `Wavetable` menus choose among them.
pub const WAVETABLES: [&str; 12] = [
    "Init Saw",
    "Basic Shapes",
    "Saw to Square",
    "Pulse Width",
    "Harmonic Sweep",
    "Odd Harmonics",
    "Vowels",
    "FM Bell",
    "Sine Fold",
    "Organ",
    "Sync Sweep",
    "Digital Steps",
];

/// What a host param is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Vital's parameter number `n` (its index in the engine's own list).
    Param(u16),
    /// The source of modulation slot `n`: 0 is none, then [`SOURCES`] in order.
    Source(u8),
    /// The destination of modulation slot `n`: 0 is none, then [`DESTINATIONS`] in order.
    Destination(u8),
    /// The wavetable of oscillator `n`: an index into [`WAVETABLES`].
    Wavetable(u8),
}

/// One host param after Enable.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// Vital's own name (`osc_1_level`).
    pub name: &'static str,
    pub role: Role,
    /// Vital's range: the normalised 0…1 travels from `min` to `max`.
    pub min: f32,
    pub max: f32,
    /// How many items the menu has (0: not a menu).
    pub menu: usize,
    /// Whether Vital counts it in whole steps.
    pub indexed: bool,
}

impl Entry {
    /// Vital's value for the normalised `norm`.
    pub fn value(&self, norm: f32) -> f32 {
        let norm = norm.clamp(0.0, 1.0);
        if self.menu > 1 {
            self.min + (norm * (self.menu - 1) as f32).round()
        } else if self.indexed {
            self.min + (norm * (self.max - self.min)).round()
        } else {
            self.min + (self.max - self.min) * norm
        }
    }
}

/// The host index (p1…) of the param Vital calls `name`.
pub fn index_of(name: &str) -> Option<usize> {
    TABLE
        .iter()
        .position(|entry| entry.name == name)
        .map(|i| i + 1)
}

#[cfg(not(target_arch = "wasm32"))]
mod ffi {
    use super::*;

    unsafe extern "C" {
        pub fn vital_new(sample_rate: f64) -> *mut c_void;
        pub fn vital_free(handle: *mut c_void);
        pub fn vital_set_bpm(handle: *mut c_void, bpm: f64);
        pub fn vital_set_param(handle: *mut c_void, index: c_int, value: f64);
        pub fn vital_get_param(handle: *mut c_void, index: c_int) -> f64;
        pub fn vital_note_on(handle: *mut c_void, note: c_int, velocity: f32, channel: c_int);
        pub fn vital_note_off(handle: *mut c_void, note: c_int, lift: f32, channel: c_int);
        pub fn vital_all_notes_off(handle: *mut c_void);
        pub fn vital_all_sound_off(handle: *mut c_void);
        pub fn vital_pitch_wheel(handle: *mut c_void, value: f32);
        pub fn vital_mod_wheel(handle: *mut c_void, value: f32);
        pub fn vital_aftertouch(handle: *mut c_void, value: f32);
        pub fn vital_set_wavetable(handle: *mut c_void, osc: c_int, kind: c_int);
        pub fn vital_mod_set_slot(
            handle: *mut c_void,
            slot: c_int,
            source: *const c_char,
            destination: *const c_char,
        ) -> c_int;
        pub fn vital_active_voices(handle: *mut c_void) -> c_int;
        pub fn vital_load_json(handle: *mut c_void, text: *const c_char, len: usize) -> c_int;
        pub fn vital_render(handle: *mut c_void, left: *mut f32, right: *mut f32, frames: u32);
    }
}

/// Frames rendered per call into the scratch.
const CHUNK: usize = 128;

/// A Vital engine.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub struct Vital {
    #[cfg(not(target_arch = "wasm32"))]
    handle: *mut c_void,
    /// Vital's value last sent for each host param (NaN before the first), so a re-applied value is not sent again:
    /// some Vital params rebuild state when set.
    sent: Vec<f32>,
    /// The (source, destination) menu choices of each modulation slot.
    slots: [(usize, usize); SLOTS],
    /// The wavetable chosen for each oscillator.
    wavetables: [usize; 3],
    tempo: f32,
    left: [f32; CHUNK],
    right: [f32; CHUNK],
}

// SAFETY: the engine is used from one thread at a time (the audio thread, after the app thread built it) and holds no
// thread-local state of its own beyond the denormal mode it sets on the thread that renders.
unsafe impl Send for Vital {}

impl Vital {
    /// A Vital engine at `sample_rate`, in its init patch (a saw on oscillator 1).
    pub fn new(sample_rate: f32) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            // SAFETY: plain construction; the handle is freed in `drop`.
            handle: unsafe { ffi::vital_new(f64::from(sample_rate)) },
            sent: vec![f32::NAN; TABLE.len()],
            slots: [(0, 0); SLOTS],
            wavetables: [0; 3],
            tempo: 0.0,
            left: [0.0; CHUNK],
            right: [0.0; CHUNK],
        }
    }

    /// Loads a Vital preset (`.vital`, JSON). 0 loaded, 1 not a Vital preset (or older than 1.0.0), 2 made by a newer
    /// Vital than this one. This builds the patch's wavetables, so it belongs on the app thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_preset(&mut self, text: &str) -> i32 {
        // SAFETY: `text` is valid for `len` bytes; the call copies what it keeps.
        let status = unsafe { ffi::vital_load_json(self.handle, text.as_ptr().cast(), text.len()) };
        self.sent.fill(f32::NAN);
        status
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_preset(&mut self, _text: &str) -> i32 {
        1
    }

    /// Vital's own current value of the parameter it numbers `vital_index` (for tests).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn engine_value(&self, vital_index: usize) -> f64 {
        // SAFETY: the handle is live until drop.
        unsafe { ffi::vital_get_param(self.handle, vital_index as c_int) }
    }

    /// How many voices are sounding.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn active_voices(&self) -> usize {
        // SAFETY: the handle is live until drop.
        unsafe { ffi::vital_active_voices(self.handle) }.max(0) as usize
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_slot(&mut self, slot: usize) {
        let (source, destination) = self.slots[slot];
        let (source, destination) = match (source.checked_sub(1), destination.checked_sub(1)) {
            (Some(source), Some(destination)) => {
                (SOURCES.get(source), DESTINATIONS.get(destination))
            }
            _ => (None, None),
        };
        let none: &CStr = c"";
        // SAFETY: the handle is live; the names are NUL-terminated and outlive the call.
        unsafe {
            ffi::vital_mod_set_slot(
                self.handle,
                slot as c_int,
                source.copied().unwrap_or(none).as_ptr(),
                destination.copied().unwrap_or(none).as_ptr(),
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Vital {
    fn drop(&mut self) {
        // SAFETY: the handle came from `vital_new` and is freed once.
        unsafe { ffi::vital_free(self.handle) };
    }
}

impl Instrument for Vital {
    fn set_parameter(&mut self, index: usize, value: f32) {
        let Some(entry) = index.checked_sub(1).and_then(|i| TABLE.get(i)) else {
            return;
        };
        let (slot_index, value) = (index - 1, entry.value(value));
        #[cfg(not(target_arch = "wasm32"))]
        match entry.role {
            Role::Param(vital_index) => {
                if self.sent[slot_index] == value {
                    return;
                }
                self.sent[slot_index] = value;
                // SAFETY: the handle is live; `vital_index` is one of the engine's own.
                unsafe {
                    ffi::vital_set_param(self.handle, c_int::from(vital_index), f64::from(value))
                };
            }
            Role::Source(slot) | Role::Destination(slot) => {
                let slot = usize::from(slot);
                let mut pair = self.slots[slot];
                match entry.role {
                    Role::Source(_) => pair.0 = value as usize,
                    _ => pair.1 = value as usize,
                }
                if pair != self.slots[slot] {
                    self.slots[slot] = pair;
                    self.set_slot(slot);
                }
            }
            Role::Wavetable(osc) => {
                let (osc, kind) = (usize::from(osc), value as usize);
                if self.wavetables[osc] != kind {
                    self.wavetables[osc] = kind;
                    // SAFETY: the handle is live.
                    unsafe { ffi::vital_set_wavetable(self.handle, osc as c_int, kind as c_int) };
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (slot_index, value);
    }

    fn note_on(&mut self, key: u8, velocity: f32, _pan: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            ffi::vital_note_on(
                self.handle,
                c_int::from(key.min(127)),
                velocity.clamp(0.0, 1.0),
                0,
            )
        };
        #[cfg(target_arch = "wasm32")]
        let _ = (key, velocity);
    }

    fn note_off(&mut self, key: u8) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            ffi::vital_note_off(self.handle, c_int::from(key.min(127)), 0.5, 0)
        };
        #[cfg(target_arch = "wasm32")]
        let _ = key;
    }

    fn all_notes_off(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            ffi::vital_all_notes_off(self.handle)
        };
    }

    fn reset(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            ffi::vital_all_sound_off(self.handle)
        };
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], context: ProcessContext) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if (context.tempo_bpm - self.tempo).abs() > 1e-3 && context.tempo_bpm > 0.0 {
                self.set_tempo_bpm(context.tempo_bpm);
            }
            let frames = left.len().min(right.len());
            let mut done = 0;
            while done < frames {
                let n = (frames - done).min(CHUNK);
                // SAFETY: the handle is live; both scratch buffers hold `CHUNK` frames.
                unsafe {
                    ffi::vital_render(
                        self.handle,
                        self.left.as_mut_ptr(),
                        self.right.as_mut_ptr(),
                        n as u32,
                    )
                };
                for i in 0..n {
                    left[done + i] += self.left[i];
                    right[done + i] += self.right[i];
                }
                done += n;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (left, right, context);
    }

    fn set_tempo_bpm(&mut self, bpm: f32) {
        self.tempo = bpm;
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            ffi::vital_set_bpm(self.handle, f64::from(bpm))
        };
    }

    fn set_controller(&mut self, index: usize, value: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: the handle is live.
        unsafe {
            match index {
                4 => ffi::vital_pitch_wheel(self.handle, (value * 2.0 - 1.0).clamp(-1.0, 1.0)),
                5 => ffi::vital_aftertouch(self.handle, value.clamp(0.0, 1.0)),
                8 => ffi::vital_mod_wheel(self.handle, value.clamp(0.0, 1.0)),
                _ => {}
            }
        };
        #[cfg(target_arch = "wasm32")]
        let _ = (index, value);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    mod ffi {
        use std::ffi::{c_char, c_int, c_void};

        unsafe extern "C" {
            pub fn vital_param_count() -> c_int;
            pub fn vital_param_name(index: c_int) -> *const c_char;
            pub fn vital_param_info(index: c_int, out: *mut f64);
            pub fn vital_param_item(index: c_int, item: c_int) -> *const c_char;
            pub fn vital_wavetable_count() -> c_int;
            pub fn vital_wavetable_name(kind: c_int) -> *const c_char;
            pub fn vital_mod_source_count(handle: *mut c_void) -> c_int;
            pub fn vital_mod_source_name(handle: *mut c_void, index: c_int) -> *const c_char;
            pub fn vital_mod_destination_count(handle: *mut c_void) -> c_int;
            pub fn vital_mod_destination_name(handle: *mut c_void, index: c_int) -> *const c_char;
        }
    }

    fn context() -> ProcessContext {
        ProcessContext {
            sample_rate: 48_000.0,
            tempo_bpm: 120.0,
            start_beat: 0.0,
            playing: true,
        }
    }

    /// Renders `blocks` blocks of 480 frames, returning the left channel.
    fn render(vital: &mut Vital, blocks: usize) -> Vec<f32> {
        let mut out = Vec::new();
        for _ in 0..blocks {
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            vital.process(&mut l, &mut r, context());
            out.extend_from_slice(&l);
        }
        out
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
    }

    /// Positive-going zero crossings.
    fn crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
    }

    fn set(vital: &mut Vital, name: &str, value: f32) {
        vital.set_parameter(index_of(name).expect(name), value);
    }

    fn cstr(p: *const c_char) -> Option<&'static str> {
        // SAFETY: the engine's names live as long as the process.
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_str().unwrap())
    }

    #[test]
    fn a_note_sounds_at_its_pitch_and_dies_away() {
        let mut vital = Vital::new(48_000.0);
        vital.note_on(69, 0.8, 0.5);
        let heard = render(&mut vital, 50); // 0.5 s
        assert!(rms(&heard[12_000..]) > 0.01, "silent");
        // A4: 440 cycles a second, on 0.25 s.
        let cycles = crossings(&heard[12_000..]);
        assert!((108..=112).contains(&cycles), "{cycles} cycles");
        vital.note_off(69);
        let tail = render(&mut vital, 100);
        assert!(rms(&tail[tail.len() - 4_800..]) < 1e-4, "still ringing");
    }

    #[test]
    fn the_host_table_is_vitals_parameter_list() {
        // SAFETY: plain queries of the engine's static list.
        let count = unsafe { ffi::vital_param_count() };
        assert_eq!(count as usize, VITAL_PARAMS);
        let mut seen = std::collections::HashSet::new();
        for (i, entry) in TABLE.iter().enumerate() {
            if let Role::Param(n) = entry.role {
                // SAFETY: as above.
                let name = cstr(unsafe { ffi::vital_param_name(c_int::from(n)) }).unwrap();
                assert_eq!(name, entry.name, "p{}", i + 1);
                assert!(seen.insert(n), "{name} twice");
                let mut info = [0.0f64; 8];
                // SAFETY: `info` holds the eight values.
                unsafe { ffi::vital_param_info(c_int::from(n), info.as_mut_ptr()) };
                assert_eq!(f64::from(entry.min), info[0], "{name} min");
                assert!(
                    (f64::from(entry.max) - info[1]).abs() <= 1e-5 * info[1].abs().max(1.0),
                    "{name} max"
                );
                assert_eq!(entry.indexed, info[3] == 0.0, "{name} indexed");
                if entry.menu > 0 && entry.name != "sub_waveform" {
                    let first = info[0] as c_int;
                    // SAFETY: as above.
                    let at = |k| unsafe { ffi::vital_param_item(c_int::from(n), k) };
                    assert!(
                        !at(first).is_null() && !at(first + entry.menu as c_int - 1).is_null(),
                        "{name}"
                    );
                    assert_eq!(entry.menu as f64, info[1] - info[0] + 1.0, "{name} items");
                }
            }
        }
    }

    #[test]
    fn the_matrix_names_are_the_engines() {
        let vital = Vital::new(48_000.0);
        // SAFETY: plain queries.
        unsafe {
            assert_eq!(
                ffi::vital_mod_source_count(vital.handle) as usize,
                SOURCES.len()
            );
            for (i, source) in SOURCES.iter().enumerate() {
                assert_eq!(
                    cstr(ffi::vital_mod_source_name(vital.handle, i as c_int)),
                    source.to_str().ok()
                );
            }
            // The engine lists the destinations that are parameters; the card shows all but the matrix's own and the tempo.
            let names: Vec<_> = (0..ffi::vital_mod_destination_count(vital.handle))
                .filter_map(|i| cstr(ffi::vital_mod_destination_name(vital.handle, i)))
                .filter(|n| !n.starts_with("modulation_") && *n != "beats_per_minute")
                .collect();
            let ours: Vec<_> = DESTINATIONS.iter().map(|d| d.to_str().unwrap()).collect();
            assert_eq!(names, ours);
        }
    }

    #[test]
    fn the_wavetables_are_the_engines() {
        // SAFETY: plain queries.
        unsafe {
            assert_eq!(ffi::vital_wavetable_count() as usize, WAVETABLES.len());
            for (i, name) in WAVETABLES.iter().enumerate() {
                assert_eq!(cstr(ffi::vital_wavetable_name(i as c_int)), Some(*name));
            }
        }
    }

    #[test]
    fn every_wavetable_sounds_at_the_pitch() {
        let wavetable = index_of("osc_1_wavetable").unwrap();
        for (kind, name) in WAVETABLES.iter().enumerate() {
            let mut vital = Vital::new(48_000.0);
            vital.set_parameter(wavetable, kind as f32 / (WAVETABLES.len() - 1) as f32);
            vital.note_on(57, 0.8, 0.5); // A3, 220 Hz
            let heard = render(&mut vital, 30);
            assert!(rms(&heard[6_000..]) > 0.005, "{} is silent", name);
            assert!(
                heard.iter().all(|v| v.is_finite() && v.abs() < 4.0),
                "{}",
                name
            );
        }
    }

    #[test]
    fn a_param_reaches_the_engine_in_its_own_units() {
        let mut vital = Vital::new(48_000.0);
        set(&mut vital, "osc_1_level", 0.25);
        let n = match TABLE[index_of("osc_1_level").unwrap() - 1].role {
            Role::Param(n) => usize::from(n),
            _ => unreachable!(),
        };
        assert!((vital.engine_value(n) - 0.25).abs() < 1e-6);
        // A menu: the unison stack style.
        set(&mut vital, "polyphony", 0.0);
        let p = match TABLE[index_of("polyphony").unwrap() - 1].role {
            Role::Param(n) => usize::from(n),
            _ => unreachable!(),
        };
        assert_eq!(vital.engine_value(p), 1.0);
    }

    #[test]
    fn a_modulation_slot_moves_a_param() {
        // LFO 1 at full depth on the oscillator's level: the level breathes, so the output is not steady.
        let mut vital = Vital::new(48_000.0);
        let source = SOURCES
            .iter()
            .position(|s| s.to_str() == Ok("lfo_1"))
            .unwrap()
            + 1;
        let destination = DESTINATIONS
            .iter()
            .position(|d| d.to_str() == Ok("osc_1_level"))
            .unwrap()
            + 1;
        set(
            &mut vital,
            "modulation_1_source",
            source as f32 / SOURCES.len() as f32,
        );
        set(
            &mut vital,
            "modulation_1_destination",
            destination as f32 / DESTINATIONS.len() as f32,
        );
        set(&mut vital, "modulation_1_amount", 1.0);
        set(&mut vital, "lfo_1_frequency", 0.75); // a quick LFO
        vital.note_on(57, 1.0, 0.5);
        let heard = render(&mut vital, 100);
        let envelope: Vec<f64> = heard[4_800..].chunks_exact(436).map(rms).collect();
        let (lo, hi) = envelope
            .iter()
            .fold((f64::MAX, 0.0f64), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        assert!(hi > 2.0 * lo.max(1e-6), "steady: {lo} .. {hi}");
        // Free again, the slot stops moving it.
        set(&mut vital, "modulation_1_destination", 0.0);
        let steady = render(&mut vital, 60);
        let envelope: Vec<f64> = steady[4_800..].chunks_exact(436).map(rms).collect();
        let (lo, hi) = envelope
            .iter()
            .fold((f64::MAX, 0.0f64), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        assert!(hi < 1.2 * lo, "still moving: {lo} .. {hi}");
    }

    #[test]
    fn a_vital_preset_loads_and_a_bad_one_is_refused() {
        let mut vital = Vital::new(48_000.0);
        assert_eq!(vital.load_preset("not json"), 1);
        assert_eq!(
            vital.load_preset(r#"{"synth_version":"9.0.0","settings":{}}"#),
            2
        );
        assert_eq!(
            vital.load_preset(r#"{"synth_version":"1.0.6","settings":{"osc_1_on":0.0}}"#),
            0
        );
        vital.note_on(60, 1.0, 0.5);
        let heard = render(&mut vital, 20);
        assert!(rms(&heard) < 1e-6, "osc 1 should be off");
        assert_eq!(
            vital.load_preset(r#"{"synth_version":"1.0.6","settings":{"osc_1_on":1.0}}"#),
            0
        );
        vital.note_on(60, 1.0, 0.5);
        assert!(rms(&render(&mut vital, 20)) > 0.01);
    }
}
