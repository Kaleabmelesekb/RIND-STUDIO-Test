use crate::wavetable::Wavetable;

pub const MAX_UNISON: usize = 8;

/// Fast, lock-free `xorshift32` PRNG returning values in `[0.0, 1.0)`.
#[inline]
pub fn xorshift32_next(state: &mut u32) -> f32 {
    let mut x = if *state == 0 { 0x9E37_79B9 } else { *state };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 * (1.0 / 16_777_216.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModOrder {
    PmThenWarp = 0,
    WarpThenPm = 1,
}

impl ModOrder {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::WarpThenPm,
            _ => Self::PmThenWarp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpMode {
    None = 0,
    BendMinus = 1,
    BendPlus = 2,
    Spectrum = 3,
}

impl WarpMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::BendMinus,
            2 => Self::BendPlus,
            3 => Self::Spectrum,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    Off = 0,
    Osc1 = 1,
    Osc2 = 2,
    Osc3 = 3,
    Osc4 = 4,
}

impl ModSource {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Osc1,
            2 => Self::Osc2,
            3 => Self::Osc3,
            4 => Self::Osc4,
            _ => Self::Off,
        }
    }

    #[inline]
    pub const fn osc_index(self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Osc1 => Some(0),
            Self::Osc2 => Some(1),
            Self::Osc3 => Some(2),
            Self::Osc4 => Some(3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModType {
    Pm = 0,
    Rm = 1,
}

impl ModType {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Rm,
            _ => Self::Pm,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Vintage2007 = 0,
    Hq = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OscPitchMode {
    #[default]
    Semitones = 0,
    Ratio = 1,
}

/// Maps normalized `[0.0, 1.0]` to symmetrically skewed `[-1000.0, +1000.0] Hz` (bias near `0.1–20 Hz`).
#[inline]
pub fn osc_hz_offset_from_norm(norm: f32) -> f32 {
    let d = (norm.clamp(0.0, 1.0) - 0.5) * 2.0;
    d.signum() * d.abs().powf(4.0) * 1000.0
}

/// Maps `[-1000.0, +1000.0] Hz` back into normalized `[0.0, 1.0]`.
#[inline]
pub fn osc_hz_offset_to_norm(hz: f32) -> f32 {
    let s = (hz / 1000.0).clamp(-1.0, 1.0);
    0.5 + 0.5 * s.signum() * s.abs().powf(0.25)
}

/// Per-oscillator control-rate / sample-rate parameters.
#[derive(Debug, Clone, Copy)]
pub struct OscParams {
    pub enabled: bool,
    pub wt_pos: f32,
    pub pitch_mode: OscPitchMode,
    pub pitch_semitones: f32,
    pub ratio: f32,
    pub hz_offset: f32,
    /// Normalized phase offset `[0.0, 1.0)` corresponding to `0°..360°`.
    pub phase_offset: f32,
    pub retrig: bool,
    pub warp_mode: WarpMode,
    pub warp_amount: f32,
    pub mod_source: ModSource,
    pub mod_type: ModType,
    pub mod_depth: f32,
    pub mod_order: ModOrder,
    pub unison: usize,
    pub detune: f32,
    pub spread: f32,
    pub pan: f32,
    pub level: f32,
    pub keytrack: bool,
}

impl Default for OscParams {
    fn default() -> Self {
        Self {
            enabled: true,
            wt_pos: 0.0,
            pitch_mode: OscPitchMode::Semitones,
            pitch_semitones: 0.0,
            ratio: 1.0,
            hz_offset: 0.0,
            phase_offset: 0.0,
            retrig: true,
            warp_mode: WarpMode::None,
            warp_amount: 0.0,
            mod_source: ModSource::Off,
            mod_type: ModType::Pm,
            mod_depth: 0.0,
            mod_order: ModOrder::PmThenWarp,
            unison: 1,
            detune: 0.2,
            spread: 0.5,
            pan: 0.0,
            level: 0.0,
            keytrack: true,
        }
    }
}

/// Pre-allocated per-voice, per-oscillator state (zero heap allocations).
#[derive(Debug, Clone)]
pub struct OscillatorState {
    pub phases: [f32; MAX_UNISON],
    /// 1-pole low-pass dampener state for backward/self cross-modulation feedback.
    pub fb_lp_state: f32,
    /// Simple 1-pole DC blocker state for feedback stability.
    pub fb_dc_x1: f32,
    pub fb_dc_y1: f32,
}

impl Default for OscillatorState {
    fn default() -> Self {
        Self::new()
    }
}

impl OscillatorState {
    pub fn new() -> Self {
        Self {
            phases: [0.0; MAX_UNISON],
            fb_lp_state: 0.0,
            fb_dc_x1: 0.0,
            fb_dc_y1: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.phases = [0.0; MAX_UNISON];
        self.fb_lp_state = 0.0;
        self.fb_dc_x1 = 0.0;
        self.fb_dc_y1 = 0.0;
    }

    /// Handles oscillator phase initialization on NoteOn.
    /// - When `retrig` is `true`: resets `phases[0]` to `0.0` (and small fixed spread offsets for
    ///   unison voices `1..MAX_UNISON`) and clears the feedback filter state.
    /// - When `retrig` is `false`: keeps phases free-running and offsets each unison voice using
    ///   `xorshift32` in `[0.0, 1.0)` so chords/unison voices have natural phase randomization.
    pub fn on_note_on(&mut self, retrig: bool, rng_state: &mut u32) {
        if retrig {
            self.fb_lp_state = 0.0;
            self.fb_dc_x1 = 0.0;
            self.fb_dc_y1 = 0.0;
            self.phases[0] = 0.0;
            for i in 1..MAX_UNISON {
                // Deterministic symmetric unison phase spread around 0.0
                let spread_offset = (i as f32 / MAX_UNISON as f32) * 0.12;
                self.phases[i] = spread_offset.fract();
            }
        } else {
            for i in 0..MAX_UNISON {
                let rand_offset = xorshift32_next(rng_state);
                self.phases[i] = (self.phases[i] + rand_offset).fract();
            }
        }
    }

    /// Processes one sample for this oscillator.
    ///
    /// Returns `(raw_mono, out_left, out_right)` where `raw_mono` is the pre-`Level`, pre-`Pan`
    /// output used as the cross-modulation tap for other oscillators.
    #[inline]
    pub fn process_sample(
        &mut self,
        osc_idx: usize,
        params: &OscParams,
        wavetable: &Wavetable,
        base_note_hz: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
        mod_signal: f32,
        is_feedback_mod: bool,
    ) -> (f32, f32, f32) {
        let _ = osc_idx;
        let sr = sample_rate.max(1000.0);

        let osc_hz = match params.pitch_mode {
            OscPitchMode::Semitones => {
                // Continuous pitch offset in semitones (-48.00..=+48.00 + modulation)
                let semitone_offset = params.pitch_semitones.clamp(-96.0, 96.0);
                base_note_hz * (2.0_f32).powf(semitone_offset / 12.0)
            }
            OscPitchMode::Ratio => (base_note_hz * params.ratio.max(0.0) + params.hz_offset).max(0.0),
        };
        let base_inc = (osc_hz / sr).clamp(0.0, 0.49);

        // Condition modulation signal (apply 1-pole feedback filter for B >= A)
        let conditioned_mod = if params.mod_source == ModSource::Off {
            self.fb_lp_state = 0.0;
            self.fb_dc_x1 = 0.0;
            self.fb_dc_y1 = 0.0;
            0.0
        } else if is_feedback_mod {
            // 1-pole feedback filter on prev_osc_out[B] when B >= A
            self.fb_lp_state += 0.5 * (mod_signal - self.fb_lp_state);
            if !self.fb_lp_state.is_finite() {
                self.fb_lp_state = 0.0;
            }
            self.fb_lp_state.clamp(-2.0, 2.0)
        } else {
            mod_signal
        };

        let phase_offset = params.phase_offset.rem_euclid(1.0).fract();

        let pm_offset = match (params.mod_source, params.mod_type) {
            (ModSource::Off, _) => 0.0,
            (_, ModType::Pm) => conditioned_mod * params.mod_depth,
            (_, ModType::Rm) => 0.0,
        };

        let unison_count = params.unison.clamp(1, MAX_UNISON);
        let max_detune_semi = params.detune.clamp(0.0, 1.0) * 0.65; // up to ±65 cents

        let mut mono_sum = 0.0_f32;
        let mut left_sum = 0.0_f32;
        let mut right_sum = 0.0_f32;

        // Normalization factor for unison energy
        let uni_norm = 1.0 / (unison_count as f32).sqrt();

        for u in 0..unison_count {
            let (detune_ratio, uni_pan) = if unison_count == 1 {
                (1.0_f32, 0.0_f32)
            } else {
                // Symmetric distribution in [-1.0, 1.0]
                let norm_pos = (u as f32 / (unison_count - 1) as f32) * 2.0 - 1.0;
                let semi_det = norm_pos * max_detune_semi;
                let ratio = (2.0_f32).powf(semi_det / 12.0);
                let pan = norm_pos * params.spread.clamp(0.0, 1.0);
                (ratio, pan)
            };

            let inc = (base_inc * detune_ratio).clamp(0.0, 0.49);
            let p_raw = (self.phases[u] + phase_offset).rem_euclid(1.0);

            let sample = sample_with_warp_and_pm(
                wavetable,
                params.wt_pos,
                p_raw,
                inc,
                params.warp_mode,
                params.warp_amount,
                pm_offset,
                params.mod_order,
                engine_mode,
            );

            self.phases[u] = (self.phases[u] + inc).rem_euclid(1.0);

            mono_sum += sample;

            // Constant-power-ish pan per unison voice
            let u_pan = uni_pan.clamp(-1.0, 1.0);
            let l_gain = ((1.0 - u_pan) * 0.5).sqrt();
            let r_gain = ((1.0 + u_pan) * 0.5).sqrt();
            left_sum += sample * l_gain;
            right_sum += sample * r_gain;
        }

        // Keep inactive unison phases advancing so raising Unison mid-note stays smooth
        for u in unison_count..MAX_UNISON {
            self.phases[u] = (self.phases[u] + base_inc).rem_euclid(1.0);
        }

        let mut raw_mono = mono_sum * uni_norm;
        let mut stereo_l = left_sum * uni_norm;
        let mut stereo_r = right_sum * uni_norm;

        // Ring Modulation (applied to both raw tap and stereo output)
        if params.mod_source != ModSource::Off && params.mod_type == ModType::Rm {
            let depth = params.mod_depth.clamp(0.0, 1.0);
            let rm_gain = (1.0 - depth) + depth * conditioned_mod;
            raw_mono *= rm_gain;
            stereo_l *= rm_gain;
            stereo_r *= rm_gain;
        }

        // Apply oscillator Pan and Level to stereo output only (raw_mono remains pre-fader!)
        let osc_pan = params.pan.clamp(-1.0, 1.0);
        let pan_l = ((1.0 - osc_pan) * 0.5).sqrt();
        let pan_r = ((1.0 + osc_pan) * 0.5).sqrt();
        let lvl = params.level.clamp(0.0, 1.0);

        let out_l = stereo_l * pan_l * lvl;
        let out_r = stereo_r * pan_r * lvl;

        (raw_mono, out_l, out_r)
    }
}

#[inline]
fn sample_wavetable_mode(
    wavetable: &Wavetable,
    wt_pos: f32,
    phase: f32,
    phase_inc: f32,
    mode: EngineMode,
) -> f32 {
    match mode {
        EngineMode::Vintage2007 => wavetable.sample_2007(wt_pos, phase),
        EngineMode::Hq => wavetable.sample_hq(wt_pos, phase, phase_inc),
    }
}

/// Applies symmetric center-pinch (`Bend+`) or end-stretch (`Bend-`) phase warping around `0.5`.
#[inline]
pub fn warp_phase_bend(phase: f32, warp_mode: WarpMode, warp_amount: f32) -> f32 {
    let amt = warp_amount.clamp(0.0, 1.0);
    if amt <= 1e-5 {
        return phase;
    }
    let u = (phase - 0.5) * 2.0;
    let exp = match warp_mode {
        WarpMode::BendPlus => 1.0 + amt * 2.5,
        WarpMode::BendMinus => 1.0 / (1.0 + amt * 2.5),
        _ => return phase,
    };
    let u_w = u.signum() * u.abs().powf(exp);
    (0.5 + 0.5 * u_w).clamp(0.0, 1.0)
}

/// Applies Phase Warp (`None`, `Bend-`, `Bend+`, `Spectrum`) and PM
/// in either `PM -> Warp` or `Warp -> PM` order.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn sample_with_warp_and_pm(
    wavetable: &Wavetable,
    wt_pos: f32,
    phase: f32,
    phase_inc: f32,
    warp_mode: WarpMode,
    warp_amount: f32,
    pm_offset: f32,
    mod_order: ModOrder,
    engine_mode: EngineMode,
) -> f32 {
    let amt = warp_amount.clamp(0.0, 1.0);
    if amt <= 1e-5 || warp_mode == WarpMode::None {
        let p = (phase + pm_offset).rem_euclid(1.0);
        return sample_wavetable_mode(wavetable, wt_pos, p, phase_inc, engine_mode);
    }

    match (mod_order, warp_mode) {
        (_, WarpMode::None) => {
            let p = (phase + pm_offset).rem_euclid(1.0);
            sample_wavetable_mode(wavetable, wt_pos, p, phase_inc, engine_mode)
        }
        (ModOrder::PmThenWarp, WarpMode::BendMinus | WarpMode::BendPlus) => {
            let p_pm = (phase + pm_offset).rem_euclid(1.0);
            let warped = warp_phase_bend(p_pm, warp_mode, amt);
            sample_wavetable_mode(wavetable, wt_pos, warped, phase_inc, engine_mode)
        }
        (ModOrder::WarpThenPm, WarpMode::BendMinus | WarpMode::BendPlus) => {
            let warped = warp_phase_bend(phase, warp_mode, amt);
            let p_pm = (warped + pm_offset).rem_euclid(1.0);
            sample_wavetable_mode(wavetable, wt_pos, p_pm, phase_inc, engine_mode)
        }
        (_, WarpMode::Spectrum) => {
            // Harmonic multiplication warp preserving fundamental pitch:
            // Crossfade between adjacent integer cycle multipliers n0 and n1
            // so phase=0/1 wraps stay 100% click-free.
            let k = 1.0 + amt * 7.0;
            let n0 = k.floor();
            let n1 = n0 + 1.0;
            let frac = k - n0;
            let (p0, p1) = match mod_order {
                ModOrder::PmThenWarp => {
                    let p_pm = (phase + pm_offset).rem_euclid(1.0);
                    ((p_pm * n0).fract(), (p_pm * n1).fract())
                }
                ModOrder::WarpThenPm => (
                    (phase * n0 + pm_offset).rem_euclid(1.0),
                    (phase * n1 + pm_offset).rem_euclid(1.0),
                ),
            };
            let s0 = sample_wavetable_mode(wavetable, wt_pos, p0, phase_inc * n0, engine_mode);
            if frac <= 1e-5 {
                s0
            } else {
                let s1 =
                    sample_wavetable_mode(wavetable, wt_pos, p1, phase_inc * n1, engine_mode);
                s0 + (s1 - s0) * frac
            }
        }
    }
}
