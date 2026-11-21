//! Dragonfly Reverb, the original plugins by Michael Willis and Rob van den Berg (GPL-3.0-or-later,
//! <https://github.com/michaelwillis/dragonfly-reverb>), as Honeydew's own effects: Hall ([`HALL`]), Room ([`ROOM`]),
//! Plate ([`PLATE`]) and Early Reflections ([`EARLY`]).
//!
//! **The DSP is theirs, unchanged**: `vendor/dragonfly-reverb` holds the plugins' own `DSP.cpp` files and the Freeverb3
//! (Teru Kamogashira) code under them, compiled into the engine by `build.rs` (a C++ compiler is all it needs, Android's
//! NDK included) and called through the C functions of `bridge.cpp`. Only DPF, the plugin framework around the DSP
//! (formats and the OpenGL window), is left out; two of its headers are replaced by a few lines each.
//!
//! **Params**, normalised, in this order (the saved one): 0 Enable, then each plugin's own parameters in its own order
//! and over its own range (the `hall`, `room`, `plate` and `early` modules below list them: the names, ranges and
//! units are the plugins'; the defaults are each plugin's default program). A [`Spec`] is one parameter's range: the
//! plugin's value is `min + (max − min)·v`; a menu (the Plate's Algorithm, the Early Reflections' Program) is item
//! `round((n − 1)·v)`. Dry and Wet levels are the plugins' own, inside the DSP; Post Gain, Mix and Wet Pan are the
//! rack's (`CommonModule`).
//!
//! Like the plugins, a size or algorithm change may allocate inside the DSP while it runs; nothing else does.
//! Where there is no C++ compiler target (the web build), the effect passes its input through.

use std::ffi::c_void;

use crate::{ExternalEffect, ProcessContext};

pub const HALL: u16 = 1100;
pub const ROOM: u16 = 1101;
pub const PLATE: u16 = 1102;
pub const EARLY: u16 = 1103;

/// How a parameter's normalised value maps to its plugin value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Law {
    Linear,
    /// A menu: the value is the item, `round((n − 1)·v)`.
    Menu,
}

/// One parameter: its name, range, law, unit text and the decimals its value shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub law: Law,
    pub unit: &'static str,
    pub decimals: u8,
}

impl Spec {
    pub const fn lin(
        name: &'static str,
        min: f64,
        max: f64,
        unit: &'static str,
        decimals: u8,
    ) -> Self {
        Self {
            name,
            min,
            max,
            law: Law::Linear,
            unit,
            decimals,
        }
    }

    /// A menu of `items` items.
    pub const fn menu(name: &'static str, items: u32) -> Self {
        Self {
            name,
            min: 0.0,
            max: (items - 1) as f64,
            law: Law::Menu,
            unit: "",
            decimals: 0,
        }
    }

    /// The plugin value of normalised `v` (clamped to 0 … 1); a menu's is its item number.
    pub fn value(&self, v: f32) -> f64 {
        let x = self.min + (self.max - self.min) * f64::from(v.clamp(0.0, 1.0));
        match self.law {
            Law::Linear => x,
            Law::Menu => x.round(),
        }
    }

    /// The normalised value of plugin value `x`.
    pub fn norm(&self, x: f64) -> f32 {
        ((x - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
    }
}

pub mod hall {
    use super::Spec;

    pub const PARAMS: usize = 19;
    /// The plugin's own default program, normalised (the Enable first).
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0, 0.800000, 0.100000, 0.200000, 0.280000, 0.500000, 0.040000, 0.900000, 0.020000,
        0.300000, 0.400000, 0.440000, 0.300000, 0.300000, 0.330000, 0.375000, 0.121212, 0.200000,
        0.150000,
    ];
    pub const DRY_LEVEL: usize = 1;
    pub const EARLY_LEVEL: usize = 2;
    pub const LATE_LEVEL: usize = 3;
    pub const SIZE: usize = 4;
    pub const WIDTH: usize = 5;
    pub const PREDELAY: usize = 6;
    pub const DIFFUSE: usize = 7;
    pub const LOW_CUT: usize = 8;
    pub const LOW_CROSS: usize = 9;
    pub const LOW_MULT: usize = 10;
    pub const HIGH_CUT: usize = 11;
    pub const HIGH_CROSS: usize = 12;
    pub const HIGH_MULT: usize = 13;
    pub const SPIN: usize = 14;
    pub const WANDER: usize = 15;
    pub const DECAY: usize = 16;
    pub const EARLY_SEND: usize = 17;
    pub const MODULATION: usize = 18;
    /// Params 1 … 18, the plugin's own ranges.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry Level", 0.0, 100.0, " %", 0),
        Spec::lin("Early Level", 0.0, 100.0, " %", 0),
        Spec::lin("Late Level", 0.0, 100.0, " %", 0),
        Spec::lin("Size", 10.0, 60.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 0),
        Spec::lin("Diffuse", 0.0, 100.0, " %", 0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::lin("Low Cross", 200.0, 1200.0, " Hz", 0),
        Spec::lin("Low Mult", 0.5, 2.5, "x", 2),
        Spec::lin("High Cut", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("High Cross", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("High Mult", 0.2, 1.2, "x", 2),
        Spec::lin("Spin", 0.0, 10.0, " Hz", 1),
        Spec::lin("Wander", 0.0, 40.0, " ms", 0),
        Spec::lin("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Early Send", 0.0, 100.0, " %", 0),
        Spec::lin("Modulation", 0.0, 100.0, " %", 0),
    ];
}

pub mod room {
    use super::Spec;

    pub const PARAMS: usize = 18;
    /// The plugin's own default program, normalised (the Enable first).
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0, 0.800000, 0.100000, 0.200000, 0.200000, 0.166667, 0.500000, 0.080000, 0.030303,
        0.700000, 0.160000, 0.400000, 1.000000, 0.600000, 0.560000, 0.500000, 0.550000, 0.020000,
    ];
    pub const DRY_LEVEL: usize = 1;
    pub const EARLY_LEVEL: usize = 2;
    pub const EARLY_SEND: usize = 3;
    pub const LATE_LEVEL: usize = 4;
    pub const SIZE: usize = 5;
    pub const WIDTH: usize = 6;
    pub const PREDELAY: usize = 7;
    pub const DECAY: usize = 8;
    pub const DIFFUSE: usize = 9;
    pub const SPIN: usize = 10;
    pub const WANDER: usize = 11;
    pub const HIGH_CUT: usize = 12;
    pub const EARLY_DAMP: usize = 13;
    pub const LATE_DAMP: usize = 14;
    pub const LOW_BOOST: usize = 15;
    pub const BOOST_FREQ: usize = 16;
    pub const LOW_CUT: usize = 17;
    /// Params 1 … 17, the plugin's own ranges.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry Level", 0.0, 100.0, " %", 0),
        Spec::lin("Early Level", 0.0, 100.0, " %", 0),
        Spec::lin("Early Send", 0.0, 100.0, " %", 0),
        Spec::lin("Late Level", 0.0, 100.0, " %", 0),
        Spec::lin("Size", 8.0, 32.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 0),
        Spec::lin("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Diffuse", 0.0, 100.0, " %", 0),
        Spec::lin("Spin", 0.0, 5.0, " Hz", 1),
        Spec::lin("Wander", 0.0, 100.0, " %", 0),
        Spec::lin("High Cut", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("Early Damp", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("Late Damp", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("Low Boost", 0.0, 100.0, " %", 0),
        Spec::lin("Boost Freq", 50.0, 1050.0, " Hz", 0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
    ];
}

pub mod plate {
    use super::Spec;

    pub const PARAMS: usize = 10;
    /// The plugin's own default program, normalised (the Enable first).
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0, 0.800000, 0.200000, 0.500000, 0.500000, 0.000000, 0.030303, 1.000000, 1.000000,
        0.800000,
    ];
    pub const DRY_LEVEL: usize = 1;
    pub const WET_LEVEL: usize = 2;
    pub const ALGORITHM: usize = 3;
    pub const WIDTH: usize = 4;
    pub const PREDELAY: usize = 5;
    pub const DECAY: usize = 6;
    pub const LOW_CUT: usize = 7;
    pub const HIGH_CUT: usize = 8;
    pub const DAMPEN: usize = 9;
    /// The Algorithm menu's items.
    pub const ALGORITHMS: [&str; 3] = ["Simple", "Nested", "Tank"];
    /// Params 1 … 9, the plugin's own ranges.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry Level", 0.0, 100.0, " %", 0),
        Spec::lin("Wet Level", 0.0, 100.0, " %", 0),
        Spec::menu("Algorithm", 3),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 0),
        Spec::lin("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::lin("High Cut", 1000.0, 16000.0, " Hz", 0),
        Spec::lin("Dampen", 1000.0, 16000.0, " Hz", 0),
    ];
}

pub mod early {
    use super::Spec;

    pub const PARAMS: usize = 8;
    /// The plugin's own default program, normalised (the Enable first).
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0, 0.800000, 0.200000, 0.285714, 0.200000, 0.500000, 0.250000, 0.600000,
    ];
    pub const DRY_LEVEL: usize = 1;
    pub const WET_LEVEL: usize = 2;
    pub const PROGRAM: usize = 3;
    pub const SIZE: usize = 4;
    pub const WIDTH: usize = 5;
    pub const LOW_CUT: usize = 6;
    pub const HIGH_CUT: usize = 7;
    /// The Program menu's items.
    pub const PROGRAM_NAMES: [&str; 8] = [
        "Abrupt Echo",
        "Backstage Pass",
        "Concert Venue",
        "Damaged Goods",
        "Elevator Pitch",
        "Floor Thirteen",
        "Garage Band",
        "Home Studio",
    ];
    /// Params 1 … 7, the plugin's own ranges.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry Level", 0.0, 100.0, " %", 0),
        Spec::lin("Wet Level", 0.0, 100.0, " %", 0),
        Spec::menu("Program", 8),
        Spec::lin("Size", 10.0, 60.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::lin("High Cut", 1000.0, 16000.0, " Hz", 0),
    ];
}

/// One plugin's C face (`vendor/dragonfly-reverb/bridge.cpp`).
#[cfg(not(target_arch = "wasm32"))]
struct Api {
    new: unsafe extern "C" fn(f64) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    set: unsafe extern "C" fn(*mut c_void, u32, f32),
    sample_rate: unsafe extern "C" fn(*mut c_void, f64),
    mute: unsafe extern "C" fn(*mut c_void),
    run: unsafe extern "C" fn(*mut c_void, *const f32, *const f32, *mut f32, *mut f32, u32),
}

#[cfg(not(target_arch = "wasm32"))]
macro_rules! plugin_api {
    ($module:ident, $new:ident, $free:ident, $set:ident, $rate:ident, $mute:ident, $run:ident) => {
        mod $module {
            use std::ffi::c_void;

            unsafe extern "C" {
                fn $new(sample_rate: f64) -> *mut c_void;
                fn $free(dsp: *mut c_void);
                fn $set(dsp: *mut c_void, index: u32, value: f32);
                fn $rate(dsp: *mut c_void, sample_rate: f64);
                fn $mute(dsp: *mut c_void);
                fn $run(
                    dsp: *mut c_void,
                    in_left: *const f32,
                    in_right: *const f32,
                    out_left: *mut f32,
                    out_right: *mut f32,
                    frames: u32,
                );
            }

            pub(super) static API: super::Api = super::Api {
                new: $new,
                free: $free,
                set: $set,
                sample_rate: $rate,
                mute: $mute,
                run: $run,
            };
        }
    };
}

#[cfg(not(target_arch = "wasm32"))]
plugin_api!(
    hall_api,
    df_hall_new,
    df_hall_free,
    df_hall_set,
    df_hall_sample_rate,
    df_hall_mute,
    df_hall_run
);
#[cfg(not(target_arch = "wasm32"))]
plugin_api!(
    room_api,
    df_room_new,
    df_room_free,
    df_room_set,
    df_room_sample_rate,
    df_room_mute,
    df_room_run
);
#[cfg(not(target_arch = "wasm32"))]
plugin_api!(
    plate_api,
    df_plate_new,
    df_plate_free,
    df_plate_set,
    df_plate_sample_rate,
    df_plate_mute,
    df_plate_run
);
#[cfg(not(target_arch = "wasm32"))]
plugin_api!(
    early_api,
    df_early_new,
    df_early_free,
    df_early_set,
    df_early_sample_rate,
    df_early_mute,
    df_early_run
);

/// The most frames the DSP runs in one call (a longer block runs in parts).
const CHUNK: usize = 1024;

/// One running Dragonfly DSP.
pub struct Dragonfly {
    #[cfg(not(target_arch = "wasm32"))]
    api: &'static Api,
    #[cfg(not(target_arch = "wasm32"))]
    dsp: *mut c_void,
    specs: &'static [Spec],
    /// The plugin value last sent to each param: the plugin rebuilds parts of its reverb on every set (even to the
    /// same value), so a re-applied value is not forwarded.
    sent: Vec<f64>,
    /// The input the DSP reads while it writes the block in place.
    in_left: Vec<f32>,
    in_right: Vec<f32>,
}

// The DSP is used by one thread at a time (the engine's), and the pointer is owned by this value alone.
unsafe impl Send for Dragonfly {}

impl Dragonfly {
    /// The DSP of module type `id` at `sample_rate`, with its params' defaults: `None` for any other type.
    pub fn new(id: u16, sample_rate: f32) -> Option<(Self, &'static [f32])> {
        let (specs, defaults): (&'static [Spec], &'static [f32]) = match id {
            HALL => (&hall::SPECS, &hall::DEFAULTS),
            ROOM => (&room::SPECS, &room::DEFAULTS),
            PLATE => (&plate::SPECS, &plate::DEFAULTS),
            EARLY => (&early::SPECS, &early::DEFAULTS),
            _ => return None,
        };
        #[cfg(not(target_arch = "wasm32"))]
        let api: &'static Api = match id {
            HALL => &hall_api::API,
            ROOM => &room_api::API,
            PLATE => &plate_api::API,
            _ => &early_api::API,
        };
        let mut dragonfly = Self {
            #[cfg(not(target_arch = "wasm32"))]
            api,
            // SAFETY: the DSP's constructor takes a sample rate and nothing else.
            #[cfg(not(target_arch = "wasm32"))]
            dsp: unsafe { (api.new)(f64::from(sample_rate)) },
            specs,
            sent: vec![f64::NAN; specs.len()],
            in_left: vec![0.0; CHUNK],
            in_right: vec![0.0; CHUNK],
        };
        let _ = sample_rate;
        for (i, v) in defaults.iter().enumerate().skip(1) {
            dragonfly.set_parameter(i, *v);
        }
        Some((dragonfly, defaults))
    }
}

impl ExternalEffect for Dragonfly {
    fn set_parameter(&mut self, index: usize, value: f32) {
        let Some(spec) = index.checked_sub(1).and_then(|i| self.specs.get(i)) else {
            return;
        };
        let plugin_value = spec.value(value) as f32;
        if self.sent[index - 1] == f64::from(plugin_value) {
            return;
        }
        self.sent[index - 1] = f64::from(plugin_value);
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: `dsp` is live until drop; the index is one of the plugin's own.
        unsafe {
            (self.api.set)(self.dsp, (index - 1) as u32, plugin_value)
        };
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _context: ProcessContext) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut start = 0;
            while start < left.len() {
                let end = (start + CHUNK).min(left.len());
                let frames = end - start;
                self.in_left[..frames].copy_from_slice(&left[start..end]);
                self.in_right[..frames].copy_from_slice(&right[start..end]);
                // SAFETY: the buffers hold `frames` samples each, and the output does not overlap the copies.
                unsafe {
                    (self.api.run)(
                        self.dsp,
                        self.in_left.as_ptr(),
                        self.in_right.as_ptr(),
                        left[start..end].as_mut_ptr(),
                        right[start..end].as_mut_ptr(),
                        frames as u32,
                    )
                };
                start = end;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (left, right, &self.in_left, &self.in_right);
    }

    fn reset(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: `dsp` is live until drop.
        unsafe {
            (self.api.mute)(self.dsp)
        };
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        // SAFETY: `dsp` is live until drop.
        unsafe {
            (self.api.sample_rate)(self.dsp, f64::from(sample_rate))
        };
        #[cfg(target_arch = "wasm32")]
        let _ = sample_rate;
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Dragonfly {
    fn drop(&mut self) {
        // SAFETY: `dsp` came from `new` and is freed once.
        unsafe { (self.api.free)(self.dsp) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 48_000.0;

    fn run(id: u16, set: &[(usize, f64)], frames: usize) -> (Vec<f32>, Vec<f32>) {
        let (mut effect, _) = Dragonfly::new(id, FS).unwrap();
        for &(i, x) in set {
            let spec = effect.specs[i - 1];
            effect.set_parameter(i, spec.norm(x));
        }
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        left[0] = 1.0;
        right[0] = 1.0;
        let context = ProcessContext {
            sample_rate: FS,
            tempo_bpm: 120.0,
            start_beat: 0.0,
            playing: false,
        };
        // In blocks, as the engine does.
        for part in 0..frames.div_ceil(500) {
            let (a, b) = (part * 500, ((part + 1) * 500).min(frames));
            effect.process(&mut left[a..b], &mut right[a..b], context);
        }
        (left, right)
    }

    fn energy(x: &[f32]) -> f64 {
        x.iter().map(|v| f64::from(*v).powi(2)).sum()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn every_reverb_rings_on_after_an_impulse_and_stays_finite() {
        for id in [HALL, ROOM, PLATE, EARLY] {
            let (left, right) = run(id, &[], 96_000);
            assert!(left.iter().chain(&right).all(|v| v.is_finite()), "{id}");
            assert!(energy(&left[2_000..]) > 1e-8, "{id} has a tail");
            assert!(
                energy(&left[90_000..]) < energy(&left[2_000..30_000]),
                "{id} decays"
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn dry_and_wet_levels_are_the_plugins_own() {
        // Dry only: the impulse comes back as it went in, with no tail.
        let (left, _) = run(
            HALL,
            &[
                (hall::DRY_LEVEL, 100.0),
                (hall::EARLY_LEVEL, 0.0),
                (hall::LATE_LEVEL, 0.0),
            ],
            4_800,
        );
        assert!((left[0] - 1.0).abs() < 1e-3);
        assert!(energy(&left[1..]) < 1e-9);
        // No dry: the first frame is silent.
        let (left, _) = run(
            PLATE,
            &[(plate::DRY_LEVEL, 0.0), (plate::WET_LEVEL, 100.0)],
            4_800,
        );
        assert!(left[0].abs() < 0.5);
        assert!(energy(&left) > 1e-6);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_longer_decay_rings_longer() {
        let tail = |s: f64| energy(&run(HALL, &[(hall::DECAY, s)], 192_000).0[96_000..]);
        assert!(tail(6.0) > tail(0.5) * 10.0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn every_parameter_runs_at_both_ends_of_its_range() {
        for (id, specs) in [
            (HALL, &hall::SPECS[..]),
            (ROOM, &room::SPECS[..]),
            (PLATE, &plate::SPECS[..]),
            (EARLY, &early::SPECS[..]),
        ] {
            for (i, spec) in specs.iter().enumerate() {
                for x in [spec.min, spec.max] {
                    let (left, right) = run(id, &[(i + 1, x)], 24_000);
                    assert!(
                        left.iter().chain(&right).all(|v| v.is_finite()),
                        "{id} {} = {x}",
                        spec.name
                    );
                }
            }
        }
    }

    #[test]
    fn values_map_over_the_plugins_ranges() {
        let size = hall::SPECS[hall::SIZE - 1];
        assert_eq!(size.value(0.0), 10.0);
        assert_eq!(size.value(1.0), 60.0);
        assert!((size.norm(35.0) - 0.5).abs() < 1e-6);
        let algorithm = plate::SPECS[plate::ALGORITHM - 1];
        assert_eq!(algorithm.value(0.5), 1.0);
        let program = early::SPECS[early::PROGRAM - 1];
        assert_eq!(program.value(1.0), 7.0);
        assert_eq!(program.value(2.0 / 7.0), 2.0);
    }

    #[test]
    fn the_defaults_are_each_plugins_default_program() {
        let value = |specs: &[Spec], defaults: &[f32], i: usize| specs[i - 1].value(defaults[i]);
        // Small Clear Hall, Medium Clear Room, Bright Plate (Nested), and the first Program that is Concert Venue.
        assert!((value(&hall::SPECS, &hall::DEFAULTS, hall::SIZE) - 24.0).abs() < 1e-3);
        assert!((value(&room::SPECS, &room::DEFAULTS, room::SIZE) - 12.0).abs() < 1e-3);
        assert_eq!(
            value(&plate::SPECS, &plate::DEFAULTS, plate::ALGORITHM),
            1.0
        );
        assert_eq!(
            early::PROGRAM_NAMES[value(&early::SPECS, &early::DEFAULTS, early::PROGRAM) as usize],
            "Concert Venue"
        );
    }
}
