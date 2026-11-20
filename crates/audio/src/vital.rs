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
    /// The normalised 0…1 of Vital's `value`: the inverse of [`Self::value`].
    pub fn norm(&self, value: f32) -> f32 {
        let span = if self.menu > 1 {
            (self.menu - 1) as f32
        } else {
            self.max - self.min
        };
        if span <= 0.0 {
            0.0
        } else {
            ((value - self.min) / span).clamp(0.0, 1.0)
        }
    }

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

/// The key a Vital engine registers under, so that its own interface finds it: the channel's id and the module's
/// instance id. 0 is no key (an engine that renders offline has none).
pub fn key(channel_id: i32, instance_id: i32) -> u64 {
    (u64::from(channel_id as u32) << 32) | u64::from(instance_id as u32)
}

/// The host index (p1…) of Vital's parameter number `vital_index`, for what its interface changed.
pub fn host_index(vital_index: usize) -> Option<usize> {
    static BY_VITAL: std::sync::LazyLock<std::collections::HashMap<u16, usize>> =
        std::sync::LazyLock::new(|| {
            TABLE
                .iter()
                .enumerate()
                .filter_map(|(i, entry)| match entry.role {
                    Role::Param(n) => Some((n, i + 1)),
                    _ => None,
                })
                .collect()
        });
    u16::try_from(vital_index)
        .ok()
        .and_then(|n| BY_VITAL.get(&n).copied())
}

/// The host index (p1…) of the source and of the destination param of modulation slot `slot`.
pub fn slot_indices(slot: usize) -> Option<(usize, usize)> {
    static BY_SLOT: std::sync::LazyLock<Vec<(usize, usize)>> = std::sync::LazyLock::new(|| {
        let mut slots = vec![(0, 0); SLOTS];
        for (i, entry) in TABLE.iter().enumerate() {
            match entry.role {
                Role::Source(n) => slots[usize::from(n)].0 = i + 1,
                Role::Destination(n) => slots[usize::from(n)].1 = i + 1,
                _ => {}
            }
        }
        slots
    });
    BY_SLOT.get(slot).copied().filter(|&(a, b)| a != 0 && b != 0)
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
        pub fn vital_new_keyed(sample_rate: f64, key: u64) -> *mut c_void;
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
        Self::with_key(sample_rate, 0)
    }

    /// As [`Self::new`], registered under `key` ([`key`]) so that Vital's own interface ([`gui`]) can open on it. 0: not.
    pub fn with_key(sample_rate: f32, key: u64) -> Self {
        let _ = key;
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            // SAFETY: plain construction; the handle is freed in `drop`.
            handle: unsafe { ffi::vital_new_keyed(f64::from(sample_rate), key) },
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

/// Vital's own interface, on Android: the original JUCE/OpenGL window, drawn windowless into a picture (`vendor/vital-ui`).
/// Everything is addressed by the engine's [`key`]; where there is no such interface (every other build) [`AVAILABLE`] is
/// false and each call does nothing.
pub mod gui {
    /// Whether this build has Vital's interface.
    pub const AVAILABLE: bool = cfg!(target_os = "android");

    /// What a frame is at most (pixels): the buffer [`take_frame`] fills.
    pub const MAX_FRAME_BYTES: usize = 2400 * 1500 * 4;

    /// A key of Vital's, for [`key`].
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Key {
        Backspace,
        Delete,
        Return,
        Escape,
        Left,
        Right,
        Up,
        Down,
        Tab,
        Home,
        End,
    }

    #[cfg(target_os = "android")]
    mod ffi {
        use std::ffi::{c_char, c_int};

        unsafe extern "C" {
            pub fn vital_gui_open(key: u64, width: c_int, height: c_int) -> c_int;
            pub fn vital_gui_close(key: u64);
            pub fn vital_gui_is_open(key: u64) -> c_int;
            pub fn vital_gui_resize(key: u64, width: c_int, height: c_int);
            pub fn vital_gui_pointer(
                key: u64,
                kind: c_int,
                x: f32,
                y: f32,
                buttons: c_int,
                pressure: f32,
                ms: i64,
            );
            pub fn vital_gui_wheel(key: u64, x: f32, y: f32, dx: f32, dy: f32, ms: i64);
            pub fn vital_gui_key(key: u64, special: c_int, text: u32, modifiers: c_int);
            pub fn vital_gui_take_frame(
                key: u64,
                dst: *mut u8,
                capacity: u64,
                width: *mut c_int,
                height: *mut c_int,
            ) -> u64;
            pub fn vital_gui_changes(
                key: u64,
                indices: *mut c_int,
                values: *mut f32,
                capacity: c_int,
            ) -> c_int;
            pub fn vital_gui_slots(key: u64, out: *mut c_char, capacity: c_int) -> c_int;
            pub fn vital_gui_wants_text(key: u64) -> c_int;
            pub fn vital_gui_set_fps(fps: c_int);
            pub fn vital_set_home(path: *const c_char);
        }
    }

    /// Opens the interface of the engine under `key` at `width` x `height` pixels. False while there is no such engine
    /// (the audio is not running) or the interface cannot be made.
    pub fn open(key: u64, width: u32, height: u32) -> bool {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        return unsafe { ffi::vital_gui_open(key, width as _, height as _) } != 0;
        #[cfg(not(target_os = "android"))]
        {
            let _ = (key, width, height);
            false
        }
    }

    pub fn close(key: u64) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_close(key)
        };
        #[cfg(not(target_os = "android"))]
        let _ = key;
    }

    /// Whether the interface is open on the engine under `key` now (an engine rebuilt under it closes it).
    pub fn is_open(key: u64) -> bool {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        return unsafe { ffi::vital_gui_is_open(key) } != 0;
        #[cfg(not(target_os = "android"))]
        {
            let _ = key;
            false
        }
    }

    pub fn resize(key: u64, width: u32, height: u32) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_resize(key, width as _, height as _)
        };
        #[cfg(not(target_os = "android"))]
        let _ = (key, width, height);
    }

    /// A finger or the pointer at (`x`, `y`) in the interface's pixels: `kind` 1 presses, 0 moves, 2 lifts; `buttons`
    /// holds JUCE's button flags (16 left, 32 right) while down.
    pub fn pointer(key: u64, kind: i32, x: f32, y: f32, buttons: i32, ms: i64) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_pointer(key, kind, x, y, buttons, 1.0, ms)
        };
        #[cfg(not(target_os = "android"))]
        let _ = (key, kind, x, y, buttons, ms);
    }

    pub fn wheel(key: u64, x: f32, y: f32, dx: f32, dy: f32, ms: i64) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_wheel(key, x, y, dx, dy, ms)
        };
        #[cfg(not(target_os = "android"))]
        let _ = (key, x, y, dx, dy, ms);
    }

    /// A typed character.
    pub fn text(key: u64, c: char) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_key(key, 0, c as u32, 0)
        };
        #[cfg(not(target_os = "android"))]
        let _ = (key, c);
    }

    /// A named key.
    pub fn key(key: u64, named: Key) {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        unsafe {
            ffi::vital_gui_key(key, named as i32 + 1, 0, 0)
        };
        #[cfg(not(target_os = "android"))]
        let _ = (key, named);
    }

    /// The newest frame, if there is one newer than the last taken: its size in pixels, with the pixels (RGBA, top row
    /// first) in `buffer`.
    pub fn take_frame(key: u64, buffer: &mut Vec<u8>) -> Option<(u32, u32)> {
        #[cfg(target_os = "android")]
        {
            if buffer.len() < MAX_FRAME_BYTES {
                buffer.resize(MAX_FRAME_BYTES, 0);
            }
            let (mut width, mut height) = (0, 0);
            // SAFETY: `buffer` holds `MAX_FRAME_BYTES`; the sizes are written.
            let id = unsafe {
                ffi::vital_gui_take_frame(
                    key,
                    buffer.as_mut_ptr(),
                    buffer.len() as u64,
                    &mut width,
                    &mut height,
                )
            };
            (id != 0 && width > 0 && height > 0).then_some((width as u32, height as u32))
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (key, buffer);
            None
        }
    }

    /// What the interface changed since the last call: Vital's parameter numbers ([`super::host_index`]) and values.
    pub fn changes(key: u64) -> Vec<(usize, f32)> {
        #[cfg(target_os = "android")]
        {
            let (mut indices, mut values) = ([0i32; 256], [0f32; 256]);
            // SAFETY: both arrays hold the capacity given.
            let n = unsafe {
                ffi::vital_gui_changes(key, indices.as_mut_ptr(), values.as_mut_ptr(), 256)
            };
            (0..n.max(0) as usize)
                .map(|i| (indices[i].max(0) as usize, values[i]))
                .collect()
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = key;
            Vec::new()
        }
    }

    /// The (source, destination) of each of the 64 modulation slots as the interface has them (empty names for a free
    /// slot), or none before the interface has looked.
    pub fn slots(key: u64) -> Option<Vec<(String, String)>> {
        #[cfg(target_os = "android")]
        {
            let mut out = vec![0u8; 16 * 1024];
            // SAFETY: `out` holds the capacity given.
            let n = unsafe { ffi::vital_gui_slots(key, out.as_mut_ptr().cast(), out.len() as i32) };
            if n <= 0 {
                return None;
            }
            out.truncate(n as usize);
            let mut names = out
                .split(|&b| b == 0)
                .map(|name| String::from_utf8_lossy(name).into_owned());
            let slots: Vec<_> = (0..super::SLOTS)
                .map(|_| (names.next().unwrap_or_default(), names.next().unwrap_or_default()))
                .collect();
            Some(slots)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = key;
            None
        }
    }

    /// Whether a text field of the interface has the keyboard.
    pub fn wants_text(key: u64) -> bool {
        #[cfg(target_os = "android")]
        // SAFETY: plain values.
        return unsafe { ffi::vital_gui_wants_text(key) } != 0;
        #[cfg(not(target_os = "android"))]
        {
            let _ = key;
            false
        }
    }

    /// The most frames per second the interface draws.
    pub fn set_fps(fps: u32) {
        #[cfg(target_os = "android")]
        // SAFETY: plain value.
        unsafe {
            ffi::vital_gui_set_fps(fps as _)
        };
        #[cfg(not(target_os = "android"))]
        let _ = fps;
    }

    /// The folder Vital keeps its presets, wavetables and settings under (its `HOME`): the app's own storage.
    pub fn set_home(path: &str) {
        #[cfg(target_os = "android")]
        if let Ok(path) = std::ffi::CString::new(path) {
            // SAFETY: `path` is NUL-terminated; the call copies it.
            unsafe { ffi::vital_set_home(path.as_ptr()) };
        }
        #[cfg(not(target_os = "android"))]
        let _ = path;
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

    #[test]
    fn norm_is_the_inverse_of_value() {
        for entry in TABLE.iter() {
            for norm in [0.0, 0.25, 0.5, 1.0] {
                let value = entry.value(norm);
                let back = entry.value(entry.norm(value));
                assert!(
                    (back - value).abs() <= 1e-4 * (1.0 + value.abs()),
                    "{}: {value} came back as {back}",
                    entry.name
                );
            }
        }
    }

    #[test]
    fn the_interface_finds_every_param_and_slot() {
        for (i, entry) in TABLE.iter().enumerate() {
            if let Role::Param(n) = entry.role {
                assert_eq!(host_index(usize::from(n)), Some(i + 1), "{}", entry.name);
            }
        }
        for slot in 0..SLOTS {
            let (source, destination) = slot_indices(slot).expect("both params");
            assert_eq!(TABLE[source - 1].role, Role::Source(slot as u8));
            assert_eq!(TABLE[destination - 1].role, Role::Destination(slot as u8));
        }
        assert_eq!(slot_indices(SLOTS), None);
    }

    #[test]
    fn a_key_is_its_two_ids() {
        assert_eq!(key(0, 0), 0);
        assert_ne!(key(1, 2), key(2, 1));
        assert_eq!(key(3, 7) >> 32, 3);
        assert_eq!(key(3, 7) & 0xffff_ffff, 7);
    }
}
