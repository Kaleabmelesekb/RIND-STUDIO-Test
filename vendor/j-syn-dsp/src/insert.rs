use std::f32::consts::TAU;

use crate::filter::fast_tanh;
use crate::oscillator::EngineMode;
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};
use crate::waveshaper::{
    mech_clip, shape_parabolic, shape_sample, shape_sine, shape_soft_clip, tube_saturate,
    DistortionMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertMode {
    Off = 0,
    Tube = 1,
    Mech = 2,
    TubeToMech = 3,
    MechToTube = 4,
    SineShaper = 5,
    Parabolic = 6,
    SoftClip = 7,
    Bitcrush = 8,
    ClassicA = 9,
    ClassicB = 10,
    ClassicC = 11,
    DiodeA = 12,
    DiodeB = 13,
    LinearFold = 14,
    SineFold = 15,
    HardClip = 16,
    HalfRectify = 17,
    FullRectify = 18,
}

impl InsertMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Tube,
            2 => Self::Mech,
            3 => Self::TubeToMech,
            4 => Self::MechToTube,
            5 => Self::SineShaper,
            6 => Self::Parabolic,
            7 => Self::SoftClip,
            8 => Self::Bitcrush,
            9 => Self::ClassicA,
            10 => Self::ClassicB,
            11 => Self::ClassicC,
            12 => Self::DiodeA,
            13 => Self::DiodeB,
            14 => Self::LinearFold,
            15 => Self::SineFold,
            16 => Self::HardClip,
            17 => Self::HalfRectify,
            18 => Self::FullRectify,
            _ => Self::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosition {
    PreFilter = 0,
    BetweenFilters = 1,
    PostFilter = 2,
}

impl InsertPosition {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::BetweenFilters,
            2 => Self::PostFilter,
            _ => Self::PreFilter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterRouting {
    Serial = 0,
    Parallel = 1,
}

impl FilterRouting {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Parallel,
            _ => Self::Serial,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InsertParams {
    pub enabled: bool,
    pub mode: InsertMode,
    pub position: InsertPosition,
    pub drive: f32, // 0.0 to 1.0
    pub mix: f32,   // 0.0 to 1.0
}

impl Default for InsertParams {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: InsertMode::Off,
            position: InsertPosition::BetweenFilters,
            drive: 0.25,
            mix: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectDest {
    #[default]
    Off = 0,
    BusA = 1,
    BusB = 2,
    Sub = 3,
}

impl DirectDest {
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::BusA,
            2 => Self::BusB,
            3 => Self::Sub,
            _ => Self::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterBusDest {
    #[default]
    BusA = 0,
    BusB = 1,
    Both = 2,
    Sub = 3,
    AC = 4,
    BC = 5,
    All = 6,
}

impl FilterBusDest {
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::BusB,
            2 => Self::Both,
            3 => Self::Sub,
            4 => Self::AC,
            5 => Self::BC,
            6 => Self::All,
            _ => Self::BusA,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RoutingParams {
    pub osc_filter_balance: [f32; 4], // -1.0 (F1) to +1.0 (F2)
    pub noise_filter_balance: f32,    // -1.0 (F1) to +1.0 (F2)
    pub osc_direct: [DirectDest; 4],
    pub noise_direct: DirectDest,
    pub filter_routing: FilterRouting,
    pub filter_out_bus: [FilterBusDest; 2],
    pub filter_mix: f32,              // -1.0 (F1) to +1.0 (F2), default 0.0
    pub inserts: [InsertParams; 2],
    pub feedback_amount: f32,         // 0.0 to 1.0
}

impl Default for RoutingParams {
    fn default() -> Self {
        Self {
            osc_filter_balance: [-1.0; 4],
            noise_filter_balance: -1.0,
            osc_direct: [DirectDest::Off; 4],
            noise_direct: DirectDest::Off,
            filter_routing: FilterRouting::Serial,
            filter_out_bus: [FilterBusDest::BusA, FilterBusDest::BusA],
            filter_mix: 0.0,
            inserts: [InsertParams::default(), InsertParams::default()],
            feedback_amount: 0.0,
        }
    }
}

/// Per-voice stereo Insert Waveshaper state (pre-allocated per voice, zero heap allocations).
#[derive(Debug, Clone, Copy)]
pub struct InsertShaperState {
    pub oversamplers: [Oversampler4x; 2],
    pub mech_low_shelf: [Biquad; 2],
    pub mech_high_shelf: [Biquad; 2],
    pub sample_rate: f32,
    pub prev_in_l: f32,
    pub prev_in_r: f32,
    pub crush_phase: f32,
    pub hold_l: f32,
    pub hold_r: f32,
}

pub type InsertState = InsertShaperState;

impl Default for InsertShaperState {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
pub fn evaluate_insert_curve(mode: InsertMode, x: f32, drive: f32) -> f32 {
    let drive = drive.clamp(0.0, 1.0);
    match mode {
        InsertMode::Off => x,
        InsertMode::Tube => tube_saturate(x, drive),
        InsertMode::Mech => mech_clip(x, drive),
        InsertMode::TubeToMech => mech_clip(tube_saturate(x, drive) * 0.75, drive) * 0.85,
        InsertMode::MechToTube => tube_saturate(mech_clip(x, drive) * 0.65, drive) * 0.90,
        InsertMode::SineShaper => shape_sine(x, drive),
        InsertMode::Parabolic => shape_parabolic(x, drive),
        InsertMode::SoftClip => shape_soft_clip(x, drive),
        InsertMode::Bitcrush => {
            let levels = 2.0_f32.powf(11.0 - drive * 8.5);
            (x.clamp(-1.5, 1.5) * levels).round() / levels
        }
        InsertMode::ClassicA => shape_sample(DistortionMode::ClassicA, x, drive, 0.0),
        InsertMode::ClassicB => shape_sample(DistortionMode::ClassicB, x, drive, 0.0),
        InsertMode::ClassicC => shape_sample(DistortionMode::ClassicC, x, drive, 0.0),
        InsertMode::DiodeA => shape_sample(DistortionMode::DiodeA, x, drive, 0.0),
        InsertMode::DiodeB => shape_sample(DistortionMode::DiodeB, x, drive, 0.0),
        InsertMode::LinearFold => shape_sample(DistortionMode::LinearFold, x, drive, 0.0),
        InsertMode::SineFold => shape_sample(DistortionMode::SineFold, x, drive, 0.0),
        InsertMode::HardClip => shape_sample(DistortionMode::HardClip, x, drive, 0.0),
        InsertMode::HalfRectify => shape_sample(DistortionMode::HalfRectify, x, drive, 0.0),
        InsertMode::FullRectify => shape_sample(DistortionMode::FullRectify, x, drive, 0.0),
    }
}

impl InsertShaperState {
    pub fn new() -> Self {
        let mut state = Self {
            oversamplers: [Oversampler4x::new(); 2],
            mech_low_shelf: [Biquad::default(); 2],
            mech_high_shelf: [Biquad::default(); 2],
            sample_rate: 44_100.0,
            prev_in_l: 0.0,
            prev_in_r: 0.0,
            crush_phase: 1.0,
            hold_l: 0.0,
            hold_r: 0.0,
        };
        state.update_mech_eq();
        state
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        self.update_mech_eq();
    }

    fn update_mech_eq(&mut self) {
        let os_rate = self.sample_rate as f64 * 4.0;
        let low_coeffs = BiquadCoeffs::low_shelf_linear(os_rate, 200.0, 0.7, 1.8);
        let high_coeffs = BiquadCoeffs::high_shelf_linear(os_rate, 4000.0, 0.7, 2.0);
        for ch in 0..2 {
            self.mech_low_shelf[ch].set_coeffs(low_coeffs);
            self.mech_high_shelf[ch].set_coeffs(high_coeffs);
        }
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.mech_low_shelf[ch].reset();
            self.mech_high_shelf[ch].reset();
        }
        self.prev_in_l = 0.0;
        self.prev_in_r = 0.0;
        self.crush_phase = 1.0;
        self.hold_l = 0.0;
        self.hold_r = 0.0;
    }

    #[inline]
    fn process_crush_channel(
        &mut self,
        ch: usize,
        input: f32,
        mode: InsertMode,
        drive: f32,
    ) -> f32 {
        let low_shelf = &mut self.mech_low_shelf[ch];
        let high_shelf = &mut self.mech_high_shelf[ch];

        self.oversamplers[ch].process_sample(input, |dry| match mode {
            InsertMode::Mech => {
                let m = high_shelf.process(low_shelf.process(dry));
                mech_clip(m, drive)
            }
            InsertMode::TubeToMech => {
                let t = tube_saturate(dry, drive) * 0.75;
                let m = high_shelf.process(low_shelf.process(t));
                mech_clip(m, drive) * 0.85
            }
            InsertMode::MechToTube => {
                let m = mech_clip(high_shelf.process(low_shelf.process(dry)), drive) * 0.65;
                tube_saturate(m, drive) * 0.90
            }
            _ => evaluate_insert_curve(mode, dry, drive),
        })
    }

    #[inline]
    pub fn process_stereo(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &InsertParams,
        engine_mode: EngineMode,
        sample_rate: f32,
    ) -> (f32, f32) {
        if (self.sample_rate - sample_rate).abs() > 0.5 {
            self.set_sample_rate(sample_rate);
        }

        if !params.enabled || params.mode == InsertMode::Off || params.mix <= 1e-5 {
            self.prev_in_l = in_l;
            self.prev_in_r = in_r;
            return (in_l, in_r);
        }

        let drive = params.drive.clamp(0.0, 1.0);
        let mix = params.mix.clamp(0.0, 1.0);

        let (wet_l, wet_r) = match params.mode {
            InsertMode::Off => (in_l, in_r),
            InsertMode::Tube
            | InsertMode::Mech
            | InsertMode::TubeToMech
            | InsertMode::MechToTube
            | InsertMode::ClassicA
            | InsertMode::ClassicB
            | InsertMode::ClassicC
            | InsertMode::DiodeA
            | InsertMode::DiodeB
            | InsertMode::LinearFold
            | InsertMode::SineFold
            | InsertMode::HardClip
            | InsertMode::HalfRectify
            | InsertMode::FullRectify => {
                self.prev_in_l = in_l;
                self.prev_in_r = in_r;
                let wl = self.process_crush_channel(0, in_l, params.mode, drive);
                let wr = self.process_crush_channel(1, in_r, params.mode, drive);
                (wl, wr)
            }
            InsertMode::SineShaper | InsertMode::Parabolic | InsertMode::SoftClip => {
                let shape_fn = match params.mode {
                    InsertMode::SineShaper => shape_sine,
                    InsertMode::Parabolic => shape_parabolic,
                    _ => shape_soft_clip,
                };
                match engine_mode {
                    EngineMode::Hq => {
                        let mid_l = 0.5 * (self.prev_in_l + in_l);
                        let mid_r = 0.5 * (self.prev_in_r + in_r);
                        let wl = 0.5 * (shape_fn(mid_l, drive) + shape_fn(in_l, drive));
                        let wr = 0.5 * (shape_fn(mid_r, drive) + shape_fn(in_r, drive));
                        self.prev_in_l = in_l;
                        self.prev_in_r = in_r;
                        (wl, wr)
                    }
                    EngineMode::Vintage2007 => {
                        self.prev_in_l = in_l;
                        self.prev_in_r = in_r;
                        (shape_fn(in_l, drive), shape_fn(in_r, drive))
                    }
                }
            }
            InsertMode::Bitcrush => {
                self.prev_in_l = in_l;
                self.prev_in_r = in_r;
                let step = 1.0 / (1.0 + drive * drive * 31.0);
                let levels = 2.0_f32.powf(11.0 - drive * 8.5);
                self.crush_phase += step;
                if self.crush_phase >= 1.0 {
                    self.crush_phase = self.crush_phase.fract();
                    self.hold_l = (in_l.clamp(-1.5, 1.5) * levels).round() / levels;
                    self.hold_r = (in_r.clamp(-1.5, 1.5) * levels).round() / levels;
                }
                (self.hold_l, self.hold_r)
            }
        };

        (
            in_l + (wet_l - in_l) * mix,
            in_r + (wet_r - in_r) * mix,
        )
    }
}

/// Per-voice 1-sample feedback path with DC blocker and 1-pole high-cut blocker.
#[derive(Debug, Clone, Copy)]
pub struct VoiceFeedbackState {
    pub prev_l: f32,
    pub prev_r: f32,
    pub dc_x_l: f32,
    pub dc_y_l: f32,
    pub dc_x_r: f32,
    pub dc_y_r: f32,
    pub lp_l: f32,
    pub lp_r: f32,
}

impl Default for VoiceFeedbackState {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceFeedbackState {
    pub fn new() -> Self {
        Self {
            prev_l: 0.0,
            prev_r: 0.0,
            dc_x_l: 0.0,
            dc_y_l: 0.0,
            dc_x_r: 0.0,
            dc_y_r: 0.0,
            lp_l: 0.0,
            lp_r: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[inline]
    pub fn read_feedback(&mut self, amount: f32, sample_rate: f32) -> (f32, f32) {
        let amt = amount.clamp(0.0, 1.0);
        if amt <= 1e-5 {
            return (0.0, 0.0);
        }

        let sr = sample_rate.max(1000.0);
        let a_lp = (1.0 - (-TAU * 6500.0 / sr).exp()).clamp(0.01, 1.0);

        let dc_l = self.prev_l - self.dc_x_l + 0.995 * self.dc_y_l;
        self.dc_x_l = self.prev_l;
        self.dc_y_l = dc_l;
        self.lp_l += (dc_l - self.lp_l) * a_lp;

        let dc_r = self.prev_r - self.dc_x_r + 0.995 * self.dc_y_r;
        self.dc_x_r = self.prev_r;
        self.dc_y_r = dc_r;
        self.lp_r += (dc_r - self.lp_r) * a_lp;

        let gain = amt * 0.95;
        (
            fast_tanh(self.lp_l * 1.2) * gain,
            fast_tanh(self.lp_r * 1.2) * gain,
        )
    }

    #[inline]
    pub fn store_post_filter(&mut self, l: f32, r: f32) {
        self.prev_l = fast_tanh(l * 0.5) * 2.0;
        self.prev_r = fast_tanh(r * 0.5) * 2.0;
    }
}
