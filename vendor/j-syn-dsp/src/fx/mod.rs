//! Parallel dual-bus 8-slot serial Master FX Rack (`Bus A` & `Bus B` + `Sub Bus`),
//! with per-slot stacking (`STACK 1..=4`) and zero audio-thread heap allocations.

pub mod diopser;
pub mod dynamics;
pub mod limiter;
pub mod modulation;
pub mod multiband_dist;
pub mod tube_resonator;
pub mod utility;

use std::f64::consts::FRAC_1_SQRT_2;

use crate::adaa::TanhAdaa2;
use crate::filter::{
    cutoff_norm_to_hz, evaluate_filter_response_db, FilterMode, FilterParams, FilterSlope,
    FilterState,
};
use crate::mod_matrix::{ModDestination, ModUiTelemetry, NUM_MOD_DESTS};
use crate::oscillator::EngineMode;
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};
use crate::waveshaper::DistortionMode;

use self::diopser::DiopserStereo;
use self::dynamics::{DistortionStereo, JSatClipperStereo, OttStereo};
use self::limiter::LimiterStereo;
use self::modulation::{ChorusStereo, DelayStereo, FlangerStereo, PhaserStereo, ReverbStereo};
use self::multiband_dist::MultiBandDistStereo;
use self::tube_resonator::TubeResonatorStereo;
use self::utility::UtilityStereo;

pub use self::diopser::{
    diopser_freq_hz, diopser_stage_count, evaluate_diopser_ripple, DIOPSER_STAGE_COUNTS,
};
pub use self::limiter::{
    limiter_ceil_db, limiter_gain_db, limiter_lookahead_ms, limiter_release_ms,
};
pub use self::modulation::{
    evaluate_flanger_scope_response, flanger_delay_ms, flanger_feedback, flanger_phase_deg,
    flanger_rate_hz, fx_high_cut_hz, fx_low_cut_hz, DELAY_SYNC_LABELS,
};
pub use self::multiband_dist::{
    mb_band_level_db, mb_band_magnitude_at_hz, mb_ordered_xover_hz, mb_out_gain_db,
    mb_xover_high_hz, mb_xover_high_norm, mb_xover_low_hz, mb_xover_low_norm, MbDistBandParams,
    MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM,
};
pub use self::tube_resonator::{
    evaluate_tube_res_scope_response, format_tube_res_tune, tube_res_decay_ms,
    tube_res_flt_freq_hz, tube_res_flt_width_oct, tube_res_lfo_rate_hz, tube_res_tune_hz,
    tube_res_tune_midi,
};
pub use self::utility::{
    format_utility_pan, utility_gain_db_from_norm, utility_pan_bipolar, utility_width_from_norm,
    UTILITY_WIDTH_DEFAULT_NORM,
};

pub const NUM_FX_BUSES: usize = 3;
pub const NUM_FX_SLOTS: usize = 8;
pub const MAX_FX_STACK: usize = 4;

const SCOPE_HZ: f32 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FxType {
    Off = 0,
    Ott = 1,
    Distortion = 2,
    Chorus = 3,
    Delay = 4,
    Reverb = 5,
    JSatClipper = 6,
    Filter = 7,
    Phaser = 8,
    Diopser = 9,
    TubeResonator = 10,
    Utility = 11,
    Limiter = 12,
    MultibandDist = 13,
    Flanger = 14,
}

impl FxType {
    pub const ALL: [Self; 15] = [
        Self::Off,
        Self::Ott,
        Self::Distortion,
        Self::Chorus,
        Self::Delay,
        Self::Reverb,
        Self::JSatClipper,
        Self::Filter,
        Self::Phaser,
        Self::Diopser,
        Self::TubeResonator,
        Self::Utility,
        Self::Limiter,
        Self::MultibandDist,
        Self::Flanger,
    ];

    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Ott,
            2 => Self::Distortion,
            3 => Self::Chorus,
            4 => Self::Delay,
            5 => Self::Reverb,
            6 => Self::JSatClipper,
            7 => Self::Filter,
            8 => Self::Phaser,
            9 => Self::Diopser,
            10 => Self::TubeResonator,
            11 => Self::Utility,
            12 => Self::Limiter,
            13 => Self::MultibandDist,
            14 => Self::Flanger,
            _ => Self::Off,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Empty",
            Self::Ott => "OTT",
            Self::Distortion => "Distortion",
            Self::Chorus => "Chorus",
            Self::Delay => "Delay",
            Self::Reverb => "Reverb",
            Self::JSatClipper => "Clipper",
            Self::Filter => "Filter",
            Self::Phaser => "Phaser",
            Self::Diopser => "Dispersion",
            Self::TubeResonator => "Tube Res.",
            Self::Utility => "Utility",
            Self::Limiter => "Limiter",
            Self::MultibandDist => "MB Dist",
            Self::Flanger => "Flanger",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Ott => "OTT",
            Self::Distortion => "DIST",
            Self::Chorus => "CHORUS",
            Self::Delay => "DELAY",
            Self::Reverb => "REVERB",
            Self::JSatClipper => "J-SAT",
            Self::Filter => "FILTER",
            Self::Phaser => "PHASER",
            Self::Diopser => "DISP",
            Self::TubeResonator => "TUBE",
            Self::Utility => "UTIL",
            Self::Limiter => "LIM",
            Self::MultibandDist => "MBDST",
            Self::Flanger => "FLANG",
        }
    }

    #[inline]
    pub const fn subtab_label(self) -> Option<&'static str> {
        match self {
            Self::Chorus | Self::Delay | Self::Reverb => Some("EQ"),
            Self::Flanger => Some("MOD"),
            Self::TubeResonator => Some("LFO"),
            Self::Limiter => Some("ADV"),
            _ => None,
        }
    }

    #[inline]
    pub const fn has_eq_subtab(self) -> bool {
        self.subtab_label().is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FxFilterSlope {
    #[default]
    Db12 = 0,
    Db24 = 1,
    Db48 = 2,
}

impl FxFilterSlope {
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Db24,
            2 => Self::Db48,
            _ => Self::Db12,
        }
    }

    #[inline]
    pub const fn to_filter_slope(self) -> FilterSlope {
        match self {
            Self::Db12 => FilterSlope::Db12,
            Self::Db24 => FilterSlope::Db24,
            Self::Db48 => FilterSlope::Db48,
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Db12 => "12 dB",
            Self::Db24 => "24 dB",
            Self::Db48 => "48 dB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxSlotParams {
    pub fx_type: FxType,
    pub enabled: bool,
    pub stack: u8,
    pub params: [f32; 4],
    pub eq_params: [f32; 4],
    pub filter_mode: FilterMode,
    pub filter_slope: FxFilterSlope,
    /// `0..=4` index into `DIOPSER_STAGE_COUNTS` (`[8, 16, 32, 64, 128]`).
    pub diopser_stages: u8,
    /// Enable the pre-resonator softening bandpass filter in `TUBE RES.` (default `true`).
    pub tube_res_filter_enabled: bool,
    /// Invert left channel phase in `UTILITY` (default `false`).
    pub utility_inv_l: bool,
    /// Invert right channel phase in `UTILITY` (default `false`).
    pub utility_inv_r: bool,
    /// Enable 4x True Peak detection in `LIMITER` (default `false`).
    pub limiter_true_peak: bool,
    /// Master Distortion waveshaping algorithm (default `DistortionMode::Tube`).
    pub distortion_mode: DistortionMode,
    /// Per-band parameters (`[Low, Mid, High]`) for `FxType::MultibandDist`.
    pub mb_bands: [MbDistBandParams; 3],
}

impl Default for FxSlotParams {
    fn default() -> Self {
        Self {
            fx_type: FxType::Off,
            enabled: false,
            stack: 1,
            params: [0.5, 0.5, 0.5, 0.5],
            eq_params: [0.0, 1.0, 0.5, 0.5],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages: 2,
            tube_res_filter_enabled: true,
            utility_inv_l: false,
            utility_inv_r: false,
            limiter_true_peak: false,
            distortion_mode: DistortionMode::Tube,
            mb_bands: [MbDistBandParams {
                mode: DistortionMode::Tube,
                enabled: true,
                solo: false,
                drive: 0.30,
                bias: 0.50,
                level: MB_BAND_LEVEL_DEFAULT_NORM,
                mix: 1.00,
            }; 3],
        }
    }
}

impl FxSlotParams {
    /// Default normalized `[P1, P2, P3, P4]` knob positions when switching to `fx_type`.
    pub const fn default_params_for_type(fx_type: FxType) -> [f32; 4] {
        match fx_type {
            FxType::Off => [0.5, 0.5, 0.5, 1.0],
            // OTT: Depth 100%, Time 1.00x (0.50), In Gain 0 dB (0.50), Out Gain 0 dB (0.50)
            FxType::Ott => [1.00, 0.50, 0.50, 0.50],
            // Distortion: Bias Neutral (0.50), Drive 30%, Tone Neutral (0.50), Mix 100%
            FxType::Distortion => [0.50, 0.30, 0.50, 1.00],
            // Chorus / Dimension: Rate ~0.5 Hz (0.35), Depth 50%, Dim 75%, Mix 100%
            FxType::Chorus => [0.35, 0.50, 0.75, 1.00],
            // Delay: Time=1/8D (0.43), Feedback 38%, Mod 15%, Mix 100%
            FxType::Delay => [0.43, 0.38, 0.15, 1.00],
            // Reverb: Size 55%, Decay 60%, Damp 45%, Mix 100%
            FxType::Reverb => [0.55, 0.60, 0.45, 1.00],
            // Clipper: Drive 0.0 dB (0.0), Ceil 0.0 dB (1.0), Knee 0%, Mix 100%
            FxType::JSatClipper => [0.00, 1.00, 0.00, 1.00],
            // Filter: Cutoff ~2.5 kHz (0.65), Res 25%, Drive/Scream 0%, Mix 100%
            FxType::Filter => [0.65, 0.25, 0.00, 1.00],
            // Phaser: Rate ~0.21 Hz (0.30), Depth 70%, Feedback 50%, Mix 100%
            FxType::Phaser => [0.30, 0.70, 0.50, 1.00],
            // Diopser: Freq ~405 Hz (0.45), Res 50%, Spread 35%, Mix 100%
            FxType::Diopser => [0.45, 0.50, 0.35, 1.00],
            // Tube Res.: Tune C1 (12/72), Flt Freq 67 Hz (0.2676), Flt Width 2.0 oct (1.5/8.5), Mix 100%
            FxType::TubeResonator => [12.0 / 72.0, 0.267_604, 1.5 / 8.5, 1.00],
            // Utility: Width 100% (0.40), Pan C (0.50), Gain 0.0 dB (0.50), Mix 100%
            FxType::Utility => [UTILITY_WIDTH_DEFAULT_NORM, 0.50, 0.50, 1.00],
            // Limiter: Gain 0.0 dB (0.0), Ceil -0.1 dB (17.9/18.0), Lookahead 1.5 ms (0.50), Mix 100%
            FxType::Limiter => [0.00, 17.9 / 18.0, 0.50, 1.00],
            // MultibandDist: Low Xov 200 Hz (0.50), High Xov ~2.5 kHz, Out Gain 0 dB (0.50), Mix 100%
            FxType::MultibandDist => [MB_XOVER_LOW_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, 0.50, 1.00],
            // Flanger: Rate ~0.20 Hz (0.35), Depth 65%, Manual Delay ~0.95 ms (0.45), Mix 100%
            FxType::Flanger => [0.35, 0.65, 0.45, 1.00],
        }
    }

    /// Default normalized secondary `[P5, P6, P7, P8]` (`EQ` / `MOD` / `LFO` / `ADV`) knob positions for `fx_type`.
    pub const fn default_eq_params_for_type(fx_type: FxType) -> [f32; 4] {
        match fx_type {
            // Chorus EQ: Low Cut 20 Hz (0.0), High Cut 20 kHz (1.0), Spread 100% (0.5), Feedback 0% (0.0)
            FxType::Chorus => [0.0, 1.0, 0.50, 0.0],
            // Delay EQ: Low Cut 20 Hz (0.0), High Cut ~8.5 kHz (0.77), Ping-Pong 0% (0.0), Width 100% (0.5)
            FxType::Delay => [0.0, 0.77, 0.0, 0.50],
            // Reverb EQ: Low Cut ~60 Hz (0.24), High Cut 20 kHz (1.0), Pre-Dly 0 ms (0.0), Width 100% (0.5)
            FxType::Reverb => [0.24, 1.0, 0.0, 0.50],
            // Flanger MOD: Feedbk +50% (0.75), Stereo Phase 90° (0.50), Low Cut 20 Hz (0.0), Tone ~11.5 kHz (0.85)
            FxType::Flanger => [0.75, 0.50, 0.0, 0.85],
            // Tube Res. LFO: LFO Amt 50%, LFO Spd 1.00 Hz (0.42625), Feedbk/Decay 50 ms (0.29922), Mix shared on P4
            FxType::TubeResonator => [0.50, 0.426_25, 0.299_22, 1.00],
            // Limiter ADV: Release ~80 ms (0.53), Knee 0% (0.0), Link 100% (1.0), Punch 100% (1.0)
            FxType::Limiter => [0.53, 0.00, 1.00, 1.00],
            _ => [0.0, 1.0, 0.50, 0.50],
        }
    }

    /// Default live plugin Init Patch FX chain: all 8 FX slots start `ON` (`enabled = true`)
    /// with `FxType::Off` (`STACK = 1`) and `P4 = 1.0` (`100%` Mix).
    pub const fn init_patch_slot(_slot_idx: usize) -> Self {
        Self {
            fx_type: FxType::Off,
            enabled: true,
            stack: 1,
            params: Self::default_params_for_type(FxType::Off),
            eq_params: [0.0, 1.0, 0.50, 0.50],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages: 2,
            tube_res_filter_enabled: true,
            utility_inv_l: false,
            utility_inv_r: false,
            limiter_true_peak: false,
            distortion_mode: DistortionMode::Tube,
            mb_bands: [MbDistBandParams {
                mode: DistortionMode::Tube,
                enabled: true,
                solo: false,
                drive: 0.30,
                bias: 0.50,
                level: MB_BAND_LEVEL_DEFAULT_NORM,
                mix: 1.00,
            }; 3],
        }
    }
}

/// Parameters for one parallel 8-slot serial FX Bus (`Bus A` or `Bus B`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxBusParams {
    pub slots: [FxSlotParams; NUM_FX_SLOTS],
    pub level: f32,
    pub pan: f32,
}

impl Default for FxBusParams {
    fn default() -> Self {
        Self {
            slots: [FxSlotParams::default(); NUM_FX_SLOTS],
            level: 1.0,
            pan: 0.0,
        }
    }
}

/// Dedicated Sub / Direct Bus bypassing `Bus A` and `Bus B` straight into Master.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubBusParams {
    pub enabled: bool,
    pub lp_cutoff_hz: f32,
    pub clip_drive_db: f32,
    pub level: f32,
}

impl Default for SubBusParams {
    fn default() -> Self {
        Self {
            enabled: false,
            lp_cutoff_hz: 2_000.0,
            clip_drive_db: 0.0,
            level: 1.0,
        }
    }
}

/// Maps normalized `0.0..=1.0` Sub Bus LP Cutoff to `20.0..=2_000.0 Hz`.
#[inline]
pub fn sub_cutoff_norm_to_hz(norm: f32) -> f32 {
    20.0 * 100.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Maps `20.0..=2_000.0 Hz` Sub Bus LP Cutoff back to normalized `0.0..=1.0`.
#[inline]
pub fn sub_cutoff_hz_to_norm(hz: f32) -> f32 {
    (hz.clamp(20.0, 2_000.0) / 20.0).log(100.0).clamp(0.0, 1.0)
}

/// How `Bus A`, `Bus B`, and `Bus C` connect inside the Master FX Rack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FxBusTopology {
    /// Parallel: `Bus A`, `Bus B`, and `Bus C` process independently and sum into Master (`A | B | C`).
    Parallel = 0,
    /// Serial: `Bus A` output feeds directly into `Bus B`, while `Bus C` stays parallel (`A -> B | C`).
    #[default]
    Serial = 1,
    /// Cascade all 3 buses (`A -> B -> C -> Master`).
    SerialAll = 2,
    /// Bus A and Bus B both feed Bus C (`(A | B) -> C`).
    ABToC = 3,
    /// Bus A feeds Bus C while Bus B goes to Master (`A -> C | B`).
    AToC = 4,
    /// Bus B feeds Bus C while Bus A goes to Master (`A | B -> C`).
    BToC = 5,
}

impl FxBusTopology {
    /// Output target of Bus A: `0 = Master`, `1 = Bus B`, `2 = Bus C`.
    #[inline]
    pub const fn bus_a_target(self) -> u8 {
        match self {
            Self::Parallel | Self::BToC => 0,
            Self::Serial | Self::SerialAll => 1,
            Self::ABToC | Self::AToC => 2,
        }
    }

    /// Output target of Bus B: `0 = Master`, `2 = Bus C`.
    #[inline]
    pub const fn bus_b_target(self) -> u8 {
        match self {
            Self::Parallel | Self::Serial | Self::AToC => 0,
            Self::SerialAll | Self::ABToC | Self::BToC => 2,
        }
    }

    #[inline]
    pub const fn from_lane_targets(a_target: u8, b_target: u8) -> Self {
        match (a_target, b_target) {
            (0, 0) => Self::Parallel,
            (1, 0) => Self::Serial,
            (1, 2) => Self::SerialAll,
            (2, 2) => Self::ABToC,
            (2, 0) => Self::AToC,
            _ => Self::BToC,
        }
    }
}

/// Full FX Rack (`Bus A` + `Bus B` + `Sub Bus` + `FxBusTopology` + `EngineMode`).
#[derive(Debug, Clone, Copy)]
pub struct FxRackParams {
    pub buses: [FxBusParams; NUM_FX_BUSES],
    pub sub: SubBusParams,
    pub topology: FxBusTopology,
    pub engine_mode: EngineMode,
}

impl Default for FxRackParams {
    fn default() -> Self {
        Self {
            buses: [FxBusParams::default(); NUM_FX_BUSES],
            sub: SubBusParams::default(),
            topology: FxBusTopology::default(),
            engine_mode: EngineMode::Vintage2007,
        }
    }
}

#[inline]
pub(crate) fn flush_denormal(x: f32) -> f32 {
    if x.is_finite() && x.abs() >= 1e-24 {
        x
    } else {
        0.0
    }
}

#[inline]
pub(crate) fn track_idle(idle_samples: &mut u32, in_l: f32, in_r: f32) -> bool {
    if in_l.abs() < 1e-8 && in_r.abs() < 1e-8 {
        *idle_samples = idle_samples.saturating_add(1);
        true
    } else {
        *idle_samples = 0;
        false
    }
}

#[inline]
pub(crate) fn butterworth_lp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::low_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
pub(crate) fn butterworth_hp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::high_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
pub(crate) fn butterworth_allpass_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::all_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

/// Full-parity multimode stereo Filter FX reusing per-voice ZDF SVF + Comb + ADAA Scream engine.
struct FxFilterStereo {
    state: FilterState,
    sample_rate: f32,
}

impl FxFilterStereo {
    fn new(sample_rate: f32) -> Self {
        Self {
            state: FilterState::new(2_500.0),
            sample_rate: sample_rate.max(1_000.0),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        self.state.reset();
    }

    #[inline]
    fn filter_params(
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
    ) -> FilterParams {
        let fc = cutoff_norm_to_hz(p[0]);
        let eff_slope = if mode.has_fixed_12db_slope() {
            FilterSlope::Db12
        } else {
            slope.to_filter_slope()
        };
        let aux = match mode {
            FilterMode::Scream => p[2].clamp(0.0, 1.0),
            FilterMode::Comb => 0.35,
            m if m.is_multi() => {
                let freq2 = (fc * 1.65).clamp(20.0, 20_000.0);
                (freq2 / 20.0).log(1000.0).clamp(0.0, 1.0)
            }
            _ => p[3].clamp(0.0, 1.0),
        };
        let drive = if mode == FilterMode::Scream {
            0.0
        } else {
            p[2].clamp(0.0, 1.0)
        };
        FilterParams {
            enabled: true,
            mode,
            slope: eff_slope,
            cutoff_hz: fc,
            resonance: p[1].clamp(0.0, 1.0),
            drive,
            keytrack: 0.0,
            aux,
        }
    }

    #[inline]
    fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
    ) -> (f32, f32, f32) {
        let fp = Self::filter_params(mode, slope, p);
        let mix = p[3].clamp(0.0, 1.0);
        let (wl, wr) = self.state.process_stereo(
            in_l,
            in_r,
            &fp,
            60.0,
            self.sample_rate,
            EngineMode::Hq,
        );
        // `Scream`, `Comb`, and multi-filters repurpose `fp.aux`, so apply Mix manually here.
        let (ol, or) = if mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi() {
            (in_l + (wl - in_l) * mix, in_r + (wr - in_r) * mix)
        } else {
            (wl, wr)
        };
        let act = ol.abs().max(or.abs()).clamp(0.0, 1.0) * mix;
        (flush_denormal(ol), flush_denormal(or), act)
    }
}

/// Evaluates steady-state magnitude response in dB (`-36.0` .. `+24.0` dB) for an FX Filter slot.
pub fn filter_response_db_from_slot(
    mode: FilterMode,
    slope: FxFilterSlope,
    p: [f32; 4],
    freq_hz: f32,
) -> f32 {
    let fp = FxFilterStereo::filter_params(mode, slope, p);
    let mix = p[3].clamp(0.0, 1.0);
    let wet_db = evaluate_filter_response_db(&fp, freq_hz);
    if (mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi()) && mix < 0.999 {
        let wet_mag = 10.0_f32.powf(wet_db / 20.0);
        let mag = (1.0 - mix) + mix * wet_mag;
        (20.0 * mag.max(1e-4).log10()).clamp(-36.0, 24.0)
    } else {
        wet_db
    }
}

/// Complete single FX processing stage holding pre-allocated DSP sub-engines.
struct StereoFxStage {
    ott: OttStereo,
    dist: DistortionStereo,
    chorus: ChorusStereo,
    delay: DelayStereo,
    reverb: ReverbStereo,
    clipper: JSatClipperStereo,
    filter: FxFilterStereo,
    phaser: PhaserStereo,
    diopser: DiopserStereo,
    tube_res: TubeResonatorStereo,
    utility: UtilityStereo,
    limiter: LimiterStereo,
    mb_dist: Box<MultiBandDistStereo>,
    flanger: FlangerStereo,
}

impl StereoFxStage {
    fn new(sample_rate: f32) -> Self {
        Self {
            ott: OttStereo::new(sample_rate),
            dist: DistortionStereo::new(sample_rate),
            chorus: ChorusStereo::new(sample_rate),
            delay: DelayStereo::new(sample_rate),
            reverb: ReverbStereo::new(sample_rate),
            clipper: JSatClipperStereo::new(),
            filter: FxFilterStereo::new(sample_rate),
            phaser: PhaserStereo::new(sample_rate),
            diopser: DiopserStereo::new(sample_rate),
            tube_res: TubeResonatorStereo::new(sample_rate),
            utility: UtilityStereo::new(sample_rate),
            limiter: LimiterStereo::new(sample_rate),
            mb_dist: Box::new(MultiBandDistStereo::new(sample_rate)),
            flanger: FlangerStereo::new(sample_rate),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.ott.set_sample_rate(sample_rate);
        self.dist.set_sample_rate(sample_rate);
        self.chorus.set_sample_rate(sample_rate);
        self.delay.set_sample_rate(sample_rate);
        self.reverb.set_sample_rate(sample_rate);
        self.filter.set_sample_rate(sample_rate);
        self.phaser.set_sample_rate(sample_rate);
        self.diopser.set_sample_rate(sample_rate);
        self.tube_res.set_sample_rate(sample_rate);
        self.utility.set_sample_rate(sample_rate);
        self.limiter.set_sample_rate(sample_rate);
        self.mb_dist.set_sample_rate(sample_rate);
        self.flanger.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.ott.reset();
        self.dist.reset();
        self.chorus.reset();
        self.delay.reset();
        self.reverb.reset();
        self.clipper.reset();
        self.filter.reset();
        self.phaser.reset();
        self.diopser.reset();
        self.tube_res.reset();
        self.utility.reset();
        self.limiter.reset();
        self.mb_dist.reset();
        self.flanger.reset();
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_type(
        &mut self,
        slot: &FxSlotParams,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        tempo_bpm: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32, f32) {
        match slot.fx_type {
            FxType::Off => (in_l, in_r, 0.0),
            FxType::Ott => self.ott.process(in_l, in_r, p),
            FxType::Distortion => self.dist.process(in_l, in_r, slot.distortion_mode, p),
            FxType::Chorus => self.chorus.process(in_l, in_r, p, eq),
            FxType::Delay => self.delay.process(in_l, in_r, p, eq, tempo_bpm),
            FxType::Reverb => self.reverb.process(in_l, in_r, p, eq),
            FxType::JSatClipper => self.clipper.process(in_l, in_r, p),
            FxType::Filter => self.filter.process(in_l, in_r, slot.filter_mode, slot.filter_slope, p),
            FxType::Phaser => self.phaser.process(in_l, in_r, p),
            FxType::Diopser => self.diopser.process(in_l, in_r, p, slot.diopser_stages as usize),
            FxType::TubeResonator => self.tube_res.process(
                in_l,
                in_r,
                p,
                eq,
                slot.tube_res_filter_enabled,
                engine_mode == EngineMode::Vintage2007,
            ),
            FxType::Utility => self.utility.process(
                in_l,
                in_r,
                p,
                slot.utility_inv_l,
                slot.utility_inv_r,
            ),
            FxType::Limiter => self.limiter.process(
                in_l,
                in_r,
                p,
                eq,
                slot.limiter_true_peak,
            ),
            FxType::MultibandDist => self.mb_dist.process(
                in_l,
                in_r,
                &slot.mb_bands,
                p,
            ),
            FxType::Flanger => self.flanger.process(in_l, in_r, p, eq),
        }
    }
}

/// Pre-allocated single 8-slot serial FX Bus (`Bus A` or `Bus B`).
struct FxBusState {
    bus_idx: usize,
    stages: Box<[[StereoFxStage; MAX_FX_STACK]]>,
    last_slot_types: [Option<FxType>; NUM_FX_SLOTS],
    scope_accum: [(f32, f32, f32, usize); NUM_FX_SLOTS],
    scope_samples_per_col: usize,
}

impl FxBusState {
    fn new(bus_idx: usize, sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        let mut stages_vec = Vec::with_capacity(NUM_FX_SLOTS);
        for _ in 0..NUM_FX_SLOTS {
            stages_vec.push(std::array::from_fn(|_| StereoFxStage::new(sr)));
        }
        Self {
            bus_idx,
            stages: stages_vec.into_boxed_slice(),
            last_slot_types: [None; NUM_FX_SLOTS],
            scope_accum: [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS],
            scope_samples_per_col: (sr / SCOPE_HZ).round().max(1.0) as usize,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.scope_samples_per_col = (sr / SCOPE_HZ).round().max(1.0) as usize;
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.set_sample_rate(sr);
            }
        }
    }

    fn reset(&mut self) {
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.reset();
            }
        }
        self.last_slot_types = [None; NUM_FX_SLOTS];
        self.scope_accum = [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS];
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        bus: &FxBusParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        engine_mode: EngineMode,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let mut l = in_l;
        let mut r = in_r;

        for (slot_idx, slot) in bus.slots.iter().enumerate() {
            match self.last_slot_types[slot_idx] {
                Some(prev) if prev != slot.fx_type => {
                    for stage in &mut self.stages[slot_idx] {
                        stage.reset();
                    }
                    self.last_slot_types[slot_idx] = Some(slot.fx_type);
                    if let Some(tel) = telemetry {
                        tel.fx_scope(self.bus_idx, slot_idx).clear();
                    }
                }
                None => {
                    self.last_slot_types[slot_idx] = Some(slot.fx_type);
                }
                Some(_) => {}
            }

            let pre_peak = l.abs().max(r.abs());
            let mut slot_activity = 0.0_f32;

            if slot.enabled && slot.fx_type != FxType::Off {
                let mut eff = [0.0_f32; 4];
                let mut eff_eq = [0.0_f32; 4];
                for p in 0..4 {
                    let mod_val = mod_offsets
                        [ModDestination::fx_bus_slot_param(self.bus_idx, slot_idx, p).index()];
                    eff[p] = (slot.params[p] + mod_val).clamp(0.0, 1.0);
                    let eq_mod = mod_offsets
                        [ModDestination::fx_bus_slot_param(self.bus_idx, slot_idx, 4 + p).index()];
                    eff_eq[p] = (slot.eq_params[p] + eq_mod).clamp(0.0, 1.0);
                }

                let stack_count = (slot.stack as usize).clamp(1, MAX_FX_STACK);
                for stage_idx in 0..stack_count {
                    let (nl, nr, act) = self.stages[slot_idx][stage_idx].process_type(
                        slot,
                        l,
                        r,
                        eff,
                        eff_eq,
                        tempo_bpm,
                        engine_mode,
                    );
                    l = nl;
                    r = nr;
                    slot_activity = slot_activity.max(act);
                }
            }

            let post_peak = l.abs().max(r.abs());
            if let Some(tel) = telemetry {
                let (ref mut acc_pre, ref mut acc_post, ref mut acc_act, ref mut cnt) =
                    self.scope_accum[slot_idx];
                *acc_pre = acc_pre.max(pre_peak);
                *acc_post = acc_post.max(post_peak);
                *acc_act = acc_act.max(slot_activity);
                *cnt += 1;
                if *cnt >= self.scope_samples_per_col {
                    tel.fx_scope(self.bus_idx, slot_idx)
                        .push(*acc_pre, *acc_post, *acc_act);
                    *acc_pre = 0.0;
                    *acc_post = 0.0;
                    *acc_act = 0.0;
                    *cnt = 0;
                }
            }
        }

        let (lvl_dest, pan_dest) = match self.bus_idx {
            0 => (ModDestination::BusALevel, ModDestination::BusAPan),
            1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
            _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
        };
        let eff_level = (bus.level + mod_offsets[lvl_dest.index()] * 2.0).clamp(0.0, 2.0);
        let eff_pan = (bus.pan + mod_offsets[pan_dest.index()] * 2.0).clamp(-1.0, 1.0);
        let gain_l = eff_level * (1.0 - eff_pan.max(0.0));
        let gain_r = eff_level * (1.0 + eff_pan.min(0.0));

        (flush_denormal(l * gain_l), flush_denormal(r * gain_r))
    }
}

/// Dedicated Sub / Direct Bus processor (`24 dB/oct LP` -> `4x ADAA2 Clipper` -> `Level`).
struct SubBusProcessor {
    lp1: [Biquad; 2],
    lp2: [Biquad; 2],
    oversamplers: [Oversampler4x; 2],
    adaa: [TanhAdaa2; 2],
    sample_rate: f32,
    last_cutoff_hz: f32,
    last_sr: f32,
    idle_samples: u32,
    scope_accum: (f32, f32, f32, usize),
    scope_samples_per_col: usize,
}

impl SubBusProcessor {
    fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            lp1: [Biquad::default(); 2],
            lp2: [Biquad::default(); 2],
            oversamplers: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            sample_rate: sr,
            last_cutoff_hz: -1.0,
            last_sr: 0.0,
            idle_samples: 1024,
            scope_accum: (0.0, 0.0, 0.0, 0),
            scope_samples_per_col: (sr / SCOPE_HZ).round().max(1.0) as usize,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.last_sr = 0.0;
        self.scope_samples_per_col = (sr / SCOPE_HZ).round().max(1.0) as usize;
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.lp1[ch].reset();
            self.lp2[ch].reset();
            self.oversamplers[ch].reset();
            self.adaa[ch].reset();
        }
        self.last_cutoff_hz = -1.0;
        self.idle_samples = 1024;
        self.scope_accum = (0.0, 0.0, 0.0, 0);
    }

    #[inline]
    fn push_scope(&mut self, telemetry: Option<&ModUiTelemetry>, pre: f32, post: f32, act: f32) {
        if let Some(tel) = telemetry {
            let (ref mut acc_pre, ref mut acc_post, ref mut acc_act, ref mut cnt) =
                self.scope_accum;
            *acc_pre = acc_pre.max(pre);
            *acc_post = acc_post.max(post);
            *acc_act = acc_act.max(act);
            *cnt += 1;
            if *cnt >= self.scope_samples_per_col {
                tel.sub_scope.push(*acc_pre, *acc_post, *acc_act);
                *acc_pre = 0.0;
                *acc_post = 0.0;
                *acc_act = 0.0;
                *cnt = 0;
            }
        }
    }

    #[inline]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        sub: &SubBusParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let level = (sub.level + mod_offsets[ModDestination::SubLevel.index()] * 2.0).clamp(0.0, 2.0);
        if !sub.enabled {
            // Sub FX bypassed: pass raw direct signal straight through scaled by SUB LEVEL.
            let out_l = flush_denormal(in_l * level);
            let out_r = flush_denormal(in_r * level);
            let raw_peak = in_l.abs().max(in_r.abs());
            let out_peak = out_l.abs().max(out_r.abs());
            self.push_scope(telemetry, raw_peak, out_peak, 0.0);
            return (out_l, out_r);
        }
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 256 {
            if let Some(tel) = telemetry {
                if self.idle_samples == 257 {
                    tel.sub_scope.clear();
                    self.scope_accum = (0.0, 0.0, 0.0, 0);
                }
            }
            return (0.0, 0.0);
        }

        let cutoff_norm =
            (sub_cutoff_hz_to_norm(sub.lp_cutoff_hz) + mod_offsets[ModDestination::SubCutoff.index()])
                .clamp(0.0, 1.0);
        let lp_off = cutoff_norm >= 0.995;
        let cutoff_hz = sub_cutoff_norm_to_hz(cutoff_norm);
        let drive_db =
            (sub.clip_drive_db + mod_offsets[ModDestination::SubDrive.index()] * 24.0).clamp(0.0, 24.0);

        if lp_off {
            self.last_cutoff_hz = -1.0;
        } else if (cutoff_hz - self.last_cutoff_hz).abs() > 0.25
            || (self.sample_rate - self.last_sr).abs() > 0.5
        {
            if self.last_cutoff_hz < 0.0 {
                for ch in 0..2 {
                    self.lp1[ch].reset();
                    self.lp2[ch].reset();
                }
            }
            let coeffs = butterworth_lp_coeffs(self.sample_rate as f64, cutoff_hz as f64);
            for ch in 0..2 {
                self.lp1[ch].set_coeffs(coeffs);
                self.lp2[ch].set_coeffs(coeffs);
            }
            self.last_cutoff_hz = cutoff_hz;
            self.last_sr = self.sample_rate;
        }

        let drive_lin = 10.0_f32.powf(drive_db * 0.05);
        let soft_knee = 0.25 + 0.75 * (drive_db / 24.0).clamp(0.0, 1.0);
        let mut out = [0.0_f32; 2];
        let mut pre_peak = 0.0_f32;
        let mut post_peak = 0.0_f32;

        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let filt = if lp_off {
                x
            } else {
                self.lp2[ch].process(self.lp1[ch].process(x))
            };
            let driven = filt * drive_lin;
            pre_peak = pre_peak.max(driven.abs());
            let adaa = &mut self.adaa[ch];
            let clipped = self.oversamplers[ch].process_sample(driven, |s| {
                let hard = s.clamp(-1.0, 1.0);
                let soft = adaa.process(s as f64) as f32;
                hard + (soft - hard) * soft_knee
            });
            post_peak = post_peak.max(clipped.abs());
            out[ch] = flush_denormal(clipped * level);
        }

        let clip_act = ((pre_peak - post_peak).max(0.0) / pre_peak.max(0.05)).clamp(0.0, 1.0);
        self.push_scope(telemetry, pre_peak, post_peak, clip_act);

        (out[0], out[1])
    }
}

/// Complete parallel dual-bus 8-slot FX Rack + dedicated Sub Bus.
pub struct FxRackState {
    buses: Box<[FxBusState]>,
    sub_bus: SubBusProcessor,
}

impl FxRackState {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            buses: (0..NUM_FX_BUSES)
                .map(|idx| FxBusState::new(idx, sr))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sub_bus: SubBusProcessor::new(sr),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.buses[0].set_sample_rate(sr);
        self.buses[1].set_sample_rate(sr);
        self.buses[2].set_sample_rate(sr);
        self.sub_bus.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.buses[0].reset();
        self.buses[1].reset();
        self.buses[2].reset();
        self.sub_bus.reset();
    }

    #[inline]
    pub fn format_distortion_submode(p0: f32) -> &'static str {
        DistortionMode::from_index(crate::waveshaper::legacy_distortion_submode_index(p0)).name()
    }

    #[inline]
    pub fn format_delay_sync(p0: f32) -> &'static str {
        DELAY_SYNC_LABELS[DelayStereo::sync_index(p0)]
    }

    #[inline]
    pub fn filter_params_from_fx(
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
    ) -> FilterParams {
        FxFilterStereo::filter_params(mode, slope, p)
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process_buses_sample(
        &mut self,
        bus_a_in: (f32, f32),
        bus_b_in: (f32, f32),
        sub_in: (f32, f32),
        rack: &FxRackParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let (sub_l, sub_r) =
            self.sub_bus
                .process_sample(sub_in.0, sub_in.1, &rack.sub, mod_offsets, telemetry);

        let (al, ar) = self.buses[0].process_sample(
            bus_a_in.0,
            bus_a_in.1,
            &rack.buses[0],
            mod_offsets,
            tempo_bpm,
            rack.engine_mode,
            telemetry,
        );

        let mut master_l = 0.0_f32;
        let mut master_r = 0.0_f32;
        let mut b_in_l = bus_b_in.0;
        let mut b_in_r = bus_b_in.1;
        let mut c_in_l = sub_l;
        let mut c_in_r = sub_r;

        match rack.topology.bus_a_target() {
            0 => {
                master_l += al;
                master_r += ar;
            }
            1 => {
                b_in_l += al;
                b_in_r += ar;
            }
            _ => {
                c_in_l += al;
                c_in_r += ar;
            }
        }

        let (bl, br) = self.buses[1].process_sample(
            b_in_l,
            b_in_r,
            &rack.buses[1],
            mod_offsets,
            tempo_bpm,
            rack.engine_mode,
            telemetry,
        );
        match rack.topology.bus_b_target() {
            0 => {
                master_l += bl;
                master_r += br;
            }
            _ => {
                c_in_l += bl;
                c_in_r += br;
            }
        }

        let (cl, cr) = self.buses[2].process_sample(
            c_in_l,
            c_in_r,
            &rack.buses[2],
            mod_offsets,
            tempo_bpm,
            rack.engine_mode,
            telemetry,
        );

        (flush_denormal(master_l + cl), flush_denormal(master_r + cr))
    }
}
