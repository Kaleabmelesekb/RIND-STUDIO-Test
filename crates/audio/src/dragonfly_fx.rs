//! Dragonfly Reverb as Honeydew effects: Hall ([`HALL`]), Room ([`ROOM`]), Plate ([`PLATE`]) and Early Reflections
//! ([`EARLY`]), Honeydew's own modules after the open-source plugins by Michael Willis and Rob van den Berg
//! (GPL-3.0-or-later, <https://github.com/michaelwillis/dragonfly-reverb>) on Teru Kamogashira's Freeverb3
//! (GPL-2.0-or-later) and Fons Adriaensen's zita-rev1. The engines are in [`crate::dragonfly`]; this file is their
//! params and the plugins' mixing.
//!
//! **Params**, normalised, in this order (the saved one): 0 Enable, then each plugin's own parameters in its own
//! order (the `hall`, `room`, `plate` and `early` modules below name them). A [`Spec`] is one parameter's law: the
//! value is `min + (max − min)·v` ([`Law::Linear`]) or `min·(max/min)^v` ([`Law::Exp`]), the plugin's units (%, m, ms,
//! Hz, s). A menu (the Plate's Algorithm, the Early Reflections' Program) is item `round((n − 1)·v)`. Post Gain, Mix and
//! Wet Pan are the rack's (`CommonModule`). The defaults are the plugins' own (Hall: Small Clear Hall, Room: Medium
//! Clear Room, Plate: Bright Plate, Early Reflections: Concert Venue).
//!
//! **Mixing**, as in the plugins: each output is `Dry·input` plus its wet levels times the wet signals (Hall and Room:
//! early and late, the late fed `Early Send` of the early reflections beside the input; Plate and Early Reflections:
//! one wet level).
//!
//! Nothing allocates while it runs: the buffers are built at the largest size when the effect is made, and a size
//! change re-uses them.

use crate::dragonfly::early::EarlyReflections;
use crate::dragonfly::hall::Zrev2;
use crate::dragonfly::plate::{Algorithm, Plate};
use crate::dragonfly::prims::Iir1;
use crate::dragonfly::room::Progenitor2;

pub const HALL: u16 = 1008;
pub const ROOM: u16 = 1009;
pub const PLATE: u16 = 1010;
pub const EARLY: u16 = 1011;

/// How a parameter's normalised value maps to its plugin value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Law {
    Linear,
    Exp,
    /// As [`Law::Exp`], a frequency in Hz (shown as Hz or kHz).
    Freq,
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

    pub const fn exp(
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
            law: Law::Exp,
            unit,
            decimals,
        }
    }

    pub const fn freq(name: &'static str, min: f64, max: f64) -> Self {
        Self {
            name,
            min,
            max,
            law: Law::Freq,
            unit: " Hz",
            decimals: 0,
        }
    }

    pub const fn menu(name: &'static str) -> Self {
        Self {
            name,
            min: 0.0,
            max: 1.0,
            law: Law::Menu,
            unit: "",
            decimals: 0,
        }
    }

    /// The plugin value of normalised `v` (clamped to 0 … 1).
    pub fn value(&self, v: f32) -> f64 {
        let v = f64::from(v.clamp(0.0, 1.0));
        match self.law {
            Law::Linear | Law::Menu => self.min + (self.max - self.min) * v,
            Law::Exp | Law::Freq => self.min * (self.max / self.min).powf(v),
        }
    }

    /// The normalised value of plugin value `x`.
    pub fn norm(&self, x: f64) -> f32 {
        let v = match self.law {
            Law::Linear | Law::Menu => (x - self.min) / (self.max - self.min),
            Law::Exp | Law::Freq => (x / self.min).ln() / (self.max / self.min).ln(),
        };
        v.clamp(0.0, 1.0) as f32
    }
}

/// The item of a menu of `len` items at normalised `v`.
pub fn menu_item(v: f32, len: usize) -> usize {
    let last = len.saturating_sub(1);
    ((v.clamp(0.0, 1.0) * last as f32).round() as usize).min(last)
}

/// The most frames a param refresh sees as "unchanged": a value that differs from the last by more than this is new.
const SAME: f32 = 1e-6;

/// The values the effect last applied; the first refresh (NaN) applies them all.
#[derive(Clone)]
struct Seen<const N: usize>([f32; N]);

impl<const N: usize> Seen<N> {
    fn new() -> Self {
        Self([f32::NAN; N])
    }

    /// Whether param `i` of `params` is new, taking it in.
    fn changed(&mut self, params: &[f32], i: usize, default: f32) -> Option<f32> {
        let v = params.get(i).copied().unwrap_or(default);
        if self.0[i].is_nan() || (self.0[i] - v).abs() > SAME {
            self.0[i] = v;
            Some(v)
        } else {
            None
        }
    }

    fn forget(&mut self) {
        self.0 = [f32::NAN; N];
    }
}

fn cut(hz: f64, fs: f32) -> f64 {
    hz.clamp(0.0, f64::from(fs) / 2.0)
}

/// Hall: Freeverb3's `zrev2` late field behind an early-reflections stage.
pub mod hall {
    use super::Spec;

    pub const PARAMS: usize = 19;
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0, 0.8, 0.1, 0.2, 0.28, 0.5, 0.04, 0.9, 0.02, 0.511392, 0.4, 0.7315, 0.614858, 0.3, 0.33,
        0.375, 0.556972, 0.2, 0.15,
    ];
    pub const DRY: usize = 1;
    pub const EARLY: usize = 2;
    pub const LATE: usize = 3;
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
    /// Params 1 … 18.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry", 0.0, 100.0, " %", 0),
        Spec::lin("Early", 0.0, 100.0, " %", 0),
        Spec::lin("Late", 0.0, 100.0, " %", 0),
        Spec::lin("Size", 10.0, 60.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 1),
        Spec::lin("Diffuse", 0.0, 100.0, " %", 0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::freq("Low Cross", 200.0, 1200.0),
        Spec::lin("Low Mult", 0.5, 2.5, "x", 2),
        Spec::freq("High Cut", 1000.0, 16000.0),
        Spec::freq("High Cross", 1000.0, 16000.0),
        Spec::lin("High Mult", 0.2, 1.2, "x", 2),
        Spec::lin("Spin", 0.0, 10.0, " Hz", 1),
        Spec::lin("Wander", 0.0, 40.0, " ms", 1),
        Spec::exp("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Early Send", 0.0, 100.0, " %", 0),
        Spec::lin("Modulation", 0.0, 100.0, " %", 0),
    ];
}

/// Room: Freeverb3's `progenitor2` late field behind an early-reflections stage.
pub mod room {
    use super::Spec;

    pub const PARAMS: usize = 18;
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0,
        0.8,
        0.1,
        0.2,
        0.2,
        0.166667,
        0.5,
        0.08,
        std::f32::consts::LOG10_2,
        0.7,
        0.16,
        0.4,
        1.0,
        0.830482,
        0.808165,
        0.5,
        0.816189,
        0.02,
    ];
    pub const DRY: usize = 1;
    pub const EARLY: usize = 2;
    pub const EARLY_SEND: usize = 3;
    pub const LATE: usize = 4;
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
    /// Params 1 … 17.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry", 0.0, 100.0, " %", 0),
        Spec::lin("Early", 0.0, 100.0, " %", 0),
        Spec::lin("Early Send", 0.0, 100.0, " %", 0),
        Spec::lin("Late", 0.0, 100.0, " %", 0),
        Spec::lin("Size", 8.0, 32.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 1),
        Spec::exp("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Diffuse", 0.0, 100.0, " %", 0),
        Spec::lin("Spin", 0.0, 5.0, " Hz", 1),
        Spec::lin("Wander", 0.0, 100.0, " %", 0),
        Spec::freq("High Cut", 1000.0, 16000.0),
        Spec::freq("Early Damp", 1000.0, 16000.0),
        Spec::freq("Late Damp", 1000.0, 16000.0),
        Spec::lin("Low Boost", 0.0, 100.0, " %", 0),
        Spec::freq("Boost Freq", 50.0, 1050.0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
    ];
}

/// Plate: Freeverb3's `nrev`, `nrevb` and `strev`.
pub mod plate {
    use super::Spec;

    pub const PARAMS: usize = 10;
    pub const DEFAULTS: [f32; PARAMS] = [
        1.0,
        0.8,
        0.2,
        0.5,
        0.5,
        0.0,
        std::f32::consts::LOG10_2,
        1.0,
        1.0,
        0.92511,
    ];
    pub const DRY: usize = 1;
    pub const WET: usize = 2;
    pub const ALGORITHM: usize = 3;
    pub const WIDTH: usize = 4;
    pub const PREDELAY: usize = 5;
    pub const DECAY: usize = 6;
    pub const LOW_CUT: usize = 7;
    pub const HIGH_CUT: usize = 8;
    pub const DAMPEN: usize = 9;
    /// The Algorithm menu's items.
    pub const ALGORITHMS: [&str; 3] = ["Simple", "Nested", "Tank"];
    /// Params 1 … 9.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry", 0.0, 100.0, " %", 0),
        Spec::lin("Wet", 0.0, 100.0, " %", 0),
        Spec::menu("Algorithm"),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Predelay", 0.0, 100.0, " ms", 1),
        Spec::exp("Decay", 0.1, 10.0, " s", 2),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::freq("High Cut", 1000.0, 16000.0),
        Spec::freq("Dampen", 1000.0, 16000.0),
    ];
}

/// Early Reflections: Freeverb3's `earlyref` alone.
pub mod early {
    use super::Spec;

    pub const PARAMS: usize = 8;
    pub const DEFAULTS: [f32; PARAMS] = [1.0, 0.8, 0.2, 0.285714, 0.2, 0.5, 0.25, 0.830482];
    pub const DRY: usize = 1;
    pub const WET: usize = 2;
    pub const PROGRAM: usize = 3;
    pub const SIZE: usize = 4;
    pub const WIDTH: usize = 5;
    pub const LOW_CUT: usize = 6;
    pub const HIGH_CUT: usize = 7;
    /// The Program menu's item names.
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
    /// The Program menu's items and the Freeverb3 preset each is.
    pub const PROGRAMS: [(&str, u32); 8] = [
        ("Abrupt Echo", 2),
        ("Backstage Pass", 18),
        ("Concert Venue", 0),
        ("Damaged Goods", 19),
        ("Elevator Pitch", 1),
        ("Floor Thirteen", 13),
        ("Garage Band", 14),
        ("Home Studio", 21),
    ];
    /// Params 1 … 7.
    pub const SPECS: [Spec; PARAMS - 1] = [
        Spec::lin("Dry", 0.0, 100.0, " %", 0),
        Spec::lin("Wet", 0.0, 100.0, " %", 0),
        Spec::menu("Program"),
        Spec::lin("Size", 10.0, 60.0, " m", 0),
        Spec::lin("Width", 50.0, 150.0, " %", 0),
        Spec::lin("Low Cut", 0.0, 200.0, " Hz", 0),
        Spec::freq("High Cut", 1000.0, 16000.0),
    ];
}

/// The lowpass and highpass the Room and Plate put on their input, as the plugins' `iir_1st` pair per channel.
#[derive(Clone)]
struct InputCut {
    lpf: [Iir1; 2],
    hpf: [Iir1; 2],
}

impl InputCut {
    fn new() -> Self {
        Self {
            lpf: [Iir1::default(); 2],
            hpf: [Iir1::default(); 2],
        }
    }

    fn set_lpf(&mut self, hz: f64, fs: f32) {
        for f in &mut self.lpf {
            f.set_lowpass(cut(hz, fs), f64::from(fs));
        }
    }

    fn set_hpf(&mut self, hz: f64, fs: f32) {
        for f in &mut self.hpf {
            f.set_highpass(cut(hz, fs), f64::from(fs));
        }
    }

    fn mute(&mut self) {
        for f in self.lpf.iter_mut().chain(self.hpf.iter_mut()) {
            f.mute();
        }
    }

    #[inline]
    fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        (
            self.lpf[0].process(self.hpf[0].process(l)),
            self.lpf[1].process(self.hpf[1].process(r)),
        )
    }
}

/// The Hall's state.
pub struct HallState {
    early: EarlyReflections,
    late: Zrev2,
    seen: Seen<{ hall::PARAMS }>,
    sample_rate: f32,
    dry: f32,
    early_level: f32,
    late_level: f32,
    early_send: f32,
}

impl HallState {
    pub fn new(params: &[f32], sample_rate: f32) -> Self {
        let fs = f64::from(sample_rate);
        let mut early = EarlyReflections::new(fs, 0);
        early.set_width(0.8);
        let mut late = Zrev2::new(fs, 0.75);
        late.set_width(1.0);
        let mut state = Self {
            early,
            late,
            seen: Seen::new(),
            sample_rate,
            dry: 0.0,
            early_level: 0.0,
            late_level: 0.0,
            early_send: 0.0,
        };
        state.refresh(params, sample_rate);
        state
    }

    /// Takes the params: what changed is applied.
    pub fn refresh(&mut self, params: &[f32], sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 0.5 {
            self.sample_rate = sample_rate;
            self.early.set_sample_rate(f64::from(sample_rate));
            self.late.set_sample_rate(f64::from(sample_rate));
            self.seen.forget();
        }
        use hall::*;
        for i in 1..PARAMS {
            let Some(v) = self.seen.changed(params, i, DEFAULTS[i]) else {
                continue;
            };
            let x = SPECS[i - 1].value(v);
            let xf = x as f32;
            match i {
                DRY => self.dry = xf / 100.0,
                EARLY => self.early_level = xf / 100.0,
                LATE => self.late_level = xf / 100.0,
                SIZE => {
                    self.early.set_rs_factor(x / 10.0);
                    self.late.set_rs_factor(x / 80.0);
                }
                WIDTH => {
                    self.early.set_width(xf / 100.0);
                    self.late.set_width(xf / 100.0);
                }
                // Freeverb does not take a zero predelay.
                PREDELAY => self.late.set_pre_delay(x.max(0.1)),
                DIFFUSE => self.late.set_diffusion(x / 140.0),
                LOW_CUT => {
                    self.early.set_output_hpf(x);
                    self.late.set_output_hpf(x);
                }
                LOW_CROSS => self.late.set_xover_low(x),
                LOW_MULT => self.late.set_rt60_factor_low(x),
                HIGH_CUT => {
                    self.early.set_output_lpf(x);
                    self.late.set_output_lpf(x);
                }
                HIGH_CROSS => self.late.set_xover_high(x),
                HIGH_MULT => self.late.set_rt60_factor_high(x),
                SPIN => self.late.set_spin(x),
                WANDER => self.late.set_wander(x),
                DECAY => self.late.set_rt60(x),
                EARLY_SEND => self.early_send = xf / 100.0,
                MODULATION => {
                    // No modulation at all misbehaves: a tenth of a percent instead.
                    let m = if x == 0.0 { 0.001 } else { xf / 100.0 };
                    self.late.set_spin_factor(m);
                    self.late.set_lfo_factor(m);
                }
                _ => {}
            }
        }
    }

    pub fn reset(&mut self) {
        self.early.mute();
        self.late.mute();
    }

    #[inline]
    pub fn process_frame(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (el, er) = self.early.tick(l, r);
        let (ll, lr) = self
            .late
            .tick(self.early_send * el + l, self.early_send * er + r);
        (
            self.dry * l + self.early_level * el + self.late_level * ll,
            self.dry * r + self.early_level * er + self.late_level * lr,
        )
    }
}

/// The Room's state.
pub struct RoomState {
    early: EarlyReflections,
    late: Progenitor2,
    input: InputCut,
    seen: Seen<{ room::PARAMS }>,
    sample_rate: f32,
    dry: f32,
    early_level: f32,
    late_level: f32,
    early_send: f32,
    size: f64,
    decay: f64,
    boost: f64,
}

impl RoomState {
    pub fn new(params: &[f32], sample_rate: f32) -> Self {
        let fs = f64::from(sample_rate);
        let mut early = EarlyReflections::new(fs, 0);
        early.set_width(0.8);
        let mut late = Progenitor2::new(fs, 3.2);
        late.set_width(1.0);
        let mut state = Self {
            early,
            late,
            input: InputCut::new(),
            seen: Seen::new(),
            sample_rate,
            dry: 0.0,
            early_level: 0.0,
            late_level: 0.0,
            early_send: 0.0,
            size: 10.0,
            decay: 1.0,
            boost: 0.0,
        };
        state.refresh(params, sample_rate);
        state
    }

    /// The low boost's gain follows Low Boost, Decay and Size together.
    fn set_bass_boost(&mut self) {
        let gain = self.boost / 20.0 / self.decay.powf(1.5) * (self.size / 10.0);
        self.late.set_bass_boost(gain as f32);
    }

    pub fn refresh(&mut self, params: &[f32], sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 0.5 {
            self.sample_rate = sample_rate;
            self.early.set_sample_rate(f64::from(sample_rate));
            self.late.set_sample_rate(f64::from(sample_rate));
            self.seen.forget();
        }
        use room::*;
        let fs = sample_rate;
        for i in 1..PARAMS {
            let Some(v) = self.seen.changed(params, i, DEFAULTS[i]) else {
                continue;
            };
            let x = SPECS[i - 1].value(v);
            let xf = x as f32;
            match i {
                DRY => self.dry = xf / 100.0,
                EARLY => self.early_level = xf / 100.0,
                EARLY_SEND => self.early_send = xf / 100.0,
                LATE => self.late_level = xf / 100.0,
                SIZE => {
                    self.size = x;
                    self.early.set_rs_factor(x / 10.0);
                    self.late.set_rs_factor(x / 10.0);
                    self.set_bass_boost();
                }
                WIDTH => {
                    self.early.set_width(xf / 120.0);
                    self.late.set_width(xf / 100.0);
                }
                PREDELAY => self.late.set_pre_delay(x.max(0.1)),
                DECAY => {
                    self.decay = x;
                    self.late.set_rt60(x);
                    self.set_bass_boost();
                }
                DIFFUSE => {
                    self.late.set_idiffusion1(x / 120.0);
                    self.late.set_odiffusion1(x / 120.0);
                }
                SPIN => {
                    self.late.set_spin(x);
                    self.late
                        .set_spin2((100.0 - (10.0 - x) * (10.0 - x)).max(0.0).sqrt() / 2.0);
                }
                WANDER => {
                    self.late.set_wander((x / 200.0 + 0.1) as f32);
                    self.late.set_wander2((x / 200.0 + 0.1) as f32);
                }
                HIGH_CUT => self.input.set_lpf(x, fs),
                EARLY_DAMP => self.early.set_output_lpf(x),
                LATE_DAMP => {
                    self.late.set_damp(x);
                    self.late.set_output_damp(x);
                }
                LOW_BOOST => {
                    self.boost = x;
                    self.set_bass_boost();
                }
                BOOST_FREQ => self.late.set_damp2(x),
                LOW_CUT => self.input.set_hpf(x, fs),
                _ => {}
            }
        }
    }

    pub fn reset(&mut self) {
        self.early.mute();
        self.late.mute();
        self.input.mute();
    }

    #[inline]
    pub fn process_frame(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (fl, fr) = self.input.process(l, r);
        let (el, er) = self.early.tick(fl, fr);
        let (ll, lr) = self
            .late
            .tick(self.early_send * el + fl, self.early_send * er + fr);
        (
            self.dry * l + self.early_level * el + self.late_level * ll,
            self.dry * r + self.early_level * er + self.late_level * lr,
        )
    }
}

/// The Plate's state.
pub struct PlateState {
    plate: Plate,
    seen: Seen<{ plate::PARAMS }>,
    sample_rate: f32,
    dry: f32,
    wet: f32,
}

impl PlateState {
    pub fn new(params: &[f32], sample_rate: f32) -> Self {
        let mut state = Self {
            plate: Plate::new(f64::from(sample_rate)),
            seen: Seen::new(),
            sample_rate,
            dry: 0.0,
            wet: 0.0,
        };
        state.refresh(params, sample_rate);
        state
    }

    pub fn refresh(&mut self, params: &[f32], sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 0.5 {
            self.sample_rate = sample_rate;
            self.plate.set_sample_rate(f64::from(sample_rate));
            self.seen.forget();
        }
        use plate::*;
        for i in 1..PARAMS {
            let Some(v) = self.seen.changed(params, i, DEFAULTS[i]) else {
                continue;
            };
            let x = SPECS[i - 1].value(v);
            let xf = x as f32;
            match i {
                DRY => self.dry = xf / 100.0,
                WET => self.wet = xf / 100.0,
                ALGORITHM => self
                    .plate
                    .set_algorithm(Algorithm::from_index(menu_item(v, ALGORITHMS.len()))),
                WIDTH => self.plate.set_width(xf / 120.0),
                PREDELAY => self.plate.set_pre_delay(x),
                DECAY => self.plate.set_rt60(x),
                LOW_CUT => self.plate.set_input_hpf(x),
                HIGH_CUT => self.plate.set_input_lpf(x),
                DAMPEN => self.plate.set_damp(x),
                _ => {}
            }
        }
    }

    pub fn reset(&mut self) {
        self.plate.mute();
    }

    #[inline]
    pub fn process_frame(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (wl, wr) = self.plate.tick(l, r);
        (self.dry * l + self.wet * wl, self.dry * r + self.wet * wr)
    }
}

/// The Early Reflections' state.
pub struct EarlyState {
    model: EarlyReflections,
    seen: Seen<{ early::PARAMS }>,
    sample_rate: f32,
    dry: f32,
    wet: f32,
}

impl EarlyState {
    pub fn new(params: &[f32], sample_rate: f32) -> Self {
        let mut model = EarlyReflections::new(f64::from(sample_rate), 0);
        model.set_width(1.0);
        let mut state = Self {
            model,
            seen: Seen::new(),
            sample_rate,
            dry: 0.0,
            wet: 0.0,
        };
        state.refresh(params, sample_rate);
        state
    }

    pub fn refresh(&mut self, params: &[f32], sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 0.5 {
            self.sample_rate = sample_rate;
            self.model.set_sample_rate(f64::from(sample_rate));
            self.seen.forget();
        }
        use early::*;
        for i in 1..PARAMS {
            let Some(v) = self.seen.changed(params, i, DEFAULTS[i]) else {
                continue;
            };
            let x = SPECS[i - 1].value(v);
            let xf = x as f32;
            match i {
                DRY => self.dry = xf / 100.0,
                WET => self.wet = xf / 100.0,
                PROGRAM => self
                    .model
                    .load_preset(PROGRAMS[menu_item(v, PROGRAMS.len())].1),
                SIZE => self.model.set_rs_factor(x / 10.0),
                WIDTH => self.model.set_width(xf / 100.0),
                LOW_CUT => self.model.set_output_hpf(x),
                HIGH_CUT => self.model.set_output_lpf(x),
                _ => {}
            }
        }
    }

    pub fn reset(&mut self) {
        self.model.mute();
    }

    #[inline]
    pub fn process_frame(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (wl, wr) = self.model.tick(l, r);
        (self.dry * l + self.wet * wl, self.dry * r + self.wet * wr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// A noise burst of `ms`, then silence to `total` frames: the effect's output energy in the tail (after the burst).
    fn tail_energy(
        mut frame: impl FnMut(f32, f32) -> (f32, f32),
        ms: f32,
        total: usize,
    ) -> (f64, bool) {
        let burst = (RATE * ms / 1000.0) as usize;
        let mut seed = 1u32;
        let (mut energy, mut finite) = (0.0f64, true);
        for i in 0..total {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = if i < burst {
                (seed >> 8) as f32 / 8_388_608.0 - 1.0
            } else {
                0.0
            };
            let (l, r) = frame(x, x * 0.7);
            finite &= l.is_finite() && r.is_finite() && l.abs() < 50.0 && r.abs() < 50.0;
            if i >= burst {
                energy += f64::from(l * l + r * r);
            }
        }
        (energy, finite)
    }

    #[test]
    fn specs_round_trip_and_the_defaults_are_in_range() {
        for (specs, defaults) in [
            (&hall::SPECS[..], &hall::DEFAULTS[..]),
            (&room::SPECS[..], &room::DEFAULTS[..]),
            (&plate::SPECS[..], &plate::DEFAULTS[..]),
            (&early::SPECS[..], &early::DEFAULTS[..]),
        ] {
            assert_eq!(specs.len() + 1, defaults.len());
            assert_eq!(defaults[0], 1.0);
            for (spec, &d) in specs.iter().zip(&defaults[1..]) {
                assert!((0.0..=1.0).contains(&d), "{}", spec.name);
                if spec.law != Law::Menu {
                    let back = spec.norm(spec.value(d));
                    assert!((back - d).abs() < 1e-5, "{}: {d} -> {back}", spec.name);
                    assert!(
                        spec.value(0.0) >= spec.min - 1e-9 && spec.value(1.0) <= spec.max + 1e-9
                    );
                }
            }
        }
    }

    #[test]
    fn the_defaults_are_the_plugins() {
        let d = |specs: &[Spec], defaults: &[f32], i: usize| specs[i - 1].value(defaults[i]);
        assert!((d(&hall::SPECS, &hall::DEFAULTS, hall::DECAY) - 1.3).abs() < 1e-4);
        assert!((d(&hall::SPECS, &hall::DEFAULTS, hall::SIZE) - 24.0).abs() < 1e-4);
        assert!((d(&room::SPECS, &room::DEFAULTS, room::DECAY) - 0.4).abs() < 1e-4);
        assert!((d(&plate::SPECS, &plate::DEFAULTS, plate::HIGH_CUT) - 16_000.0).abs() < 1e-2);
        assert_eq!(menu_item(plate::DEFAULTS[plate::ALGORITHM], 3), 1);
        assert_eq!(menu_item(early::DEFAULTS[early::PROGRAM], 8), 2);
    }

    #[test]
    fn every_effect_rings_after_a_burst_stays_finite_and_dies_away() {
        let mut hall = HallState::new(&hall::DEFAULTS, RATE);
        let mut room = RoomState::new(&room::DEFAULTS, RATE);
        let mut plate = PlateState::new(&plate::DEFAULTS, RATE);
        let mut early = EarlyState::new(&early::DEFAULTS, RATE);
        for (name, (energy, finite)) in [
            (
                "hall",
                tail_energy(|l, r| hall.process_frame(l, r), 50.0, 96_000),
            ),
            (
                "room",
                tail_energy(|l, r| room.process_frame(l, r), 50.0, 96_000),
            ),
            (
                "plate",
                tail_energy(|l, r| plate.process_frame(l, r), 50.0, 96_000),
            ),
            (
                "early",
                tail_energy(|l, r| early.process_frame(l, r), 50.0, 96_000),
            ),
        ] {
            assert!(finite, "{name}");
            assert!(energy > 1e-6, "{name} has no tail: {energy}");
        }
    }

    #[test]
    fn a_longer_decay_leaves_more_tail() {
        let tail = |decay: f32| {
            let mut p = hall::DEFAULTS;
            p[hall::DECAY] = hall::SPECS[hall::DECAY - 1].norm(f64::from(decay));
            p[hall::DRY] = 0.0;
            let mut s = HallState::new(&p, RATE);
            // Energy well after the burst (the second half of 4 s).
            let (mut e, mut i) = (0.0f64, 0usize);
            let _ = tail_energy(
                |l, r| {
                    let out = s.process_frame(l, r);
                    i += 1;
                    if i > 96_000 {
                        e += f64::from(out.0 * out.0);
                    }
                    out
                },
                20.0,
                192_000,
            );
            e
        };
        assert!(tail(4.0) > tail(0.5) * 4.0);
    }

    #[test]
    fn the_plate_algorithms_and_the_early_programs_all_run() {
        for alg in 0..3 {
            let mut p = plate::DEFAULTS;
            p[plate::ALGORITHM] = alg as f32 / 2.0;
            let mut s = PlateState::new(&p, RATE);
            let (e, finite) = tail_energy(|l, r| s.process_frame(l, r), 30.0, 48_000);
            assert!(finite && e > 1e-6, "algorithm {alg}");
        }
        for prog in 0..8 {
            let mut p = early::DEFAULTS;
            p[early::PROGRAM] = prog as f32 / 7.0;
            let mut s = EarlyState::new(&p, RATE);
            let (e, finite) = tail_energy(|l, r| s.process_frame(l, r), 30.0, 24_000);
            assert!(finite && e > 1e-6, "program {prog}");
        }
    }

    #[test]
    fn sweeping_every_param_to_its_ends_stays_finite() {
        let ends = [0.0f32, 1.0, 0.5];
        for (i, &end) in (1..hall::PARAMS).flat_map(|i| ends.iter().map(move |e| (i, e))) {
            let mut p = hall::DEFAULTS;
            p[i] = end;
            let mut s = HallState::new(&p, RATE);
            assert!(
                tail_energy(|l, r| s.process_frame(l, r), 10.0, 12_000).1,
                "hall {i} {end}"
            );
        }
        for (i, &end) in (1..room::PARAMS).flat_map(|i| ends.iter().map(move |e| (i, e))) {
            let mut p = room::DEFAULTS;
            p[i] = end;
            let mut s = RoomState::new(&p, RATE);
            assert!(
                tail_energy(|l, r| s.process_frame(l, r), 10.0, 12_000).1,
                "room {i} {end}"
            );
        }
        for (i, &end) in (1..plate::PARAMS).flat_map(|i| ends.iter().map(move |e| (i, e))) {
            let mut p = plate::DEFAULTS;
            p[i] = end;
            let mut s = PlateState::new(&p, RATE);
            assert!(
                tail_energy(|l, r| s.process_frame(l, r), 10.0, 12_000).1,
                "plate {i} {end}"
            );
        }
    }

    #[test]
    fn changing_a_param_while_running_moves_the_sound_without_blowing_up() {
        let mut s = RoomState::new(&room::DEFAULTS, RATE);
        let mut p = room::DEFAULTS;
        let mut peak = 0.0f32;
        for block in 0..200 {
            p[room::SIZE] = (block % 7) as f32 / 6.0;
            p[room::DECAY] = (block % 5) as f32 / 4.0;
            s.refresh(&p, RATE);
            for n in 0..64 {
                let x = if (block * 64 + n) % 997 == 0 {
                    0.8
                } else {
                    0.0
                };
                let (l, r) = s.process_frame(x, x);
                assert!(l.is_finite() && r.is_finite());
                peak = peak.max(l.abs()).max(r.abs());
            }
        }
        assert!(peak < 20.0, "{peak}");
    }
}
