//! Zero-allocation stereo cascaded allpass phase-dispersion FX (`Diopser`).
//!
//! Pre-allocates 128 2nd-order biquad allpass stages per stereo channel once during setup.

use crate::oversample::{Biquad, BiquadCoeffs};

pub const MAX_DIOPSER_STAGES: usize = 128;
pub const DIOPSER_STAGE_COUNTS: [usize; 5] = [8, 16, 32, 64, 128];

#[inline]
pub const fn diopser_stage_count(stages_idx: usize) -> usize {
    if stages_idx < DIOPSER_STAGE_COUNTS.len() {
        DIOPSER_STAGE_COUNTS[stages_idx]
    } else {
        DIOPSER_STAGE_COUNTS[4]
    }
}

/// Maps normalized `0.0..=1.0` center frequency to `20.0..=16_000.0 Hz`.
#[inline]
pub fn diopser_freq_hz(norm: f32) -> f32 {
    20.0 * 800.0_f32.powf(norm.clamp(0.0, 1.0))
}

#[inline]
pub fn diopser_res_to_q(res: f32) -> f64 {
    let r = res.clamp(0.0, 1.0) as f64;
    0.707_106_78 + r * r * 14.0
}

#[inline]
pub fn diopser_stage_freq_hz(
    center_hz: f32,
    spread: f32,
    stage_idx: usize,
    num_stages: usize,
    max_hz: f32,
) -> f32 {
    let n = num_stages.max(1) as f32;
    let u = ((stage_idx as f32 + 0.5) / n) * 2.0 - 1.0;
    let span_oct = spread.clamp(0.0, 1.0) * 2.5;
    (center_hz * 2.0_f32.powf(u * span_oct)).clamp(20.0, max_hz)
}

/// Evaluates normalized `0.0..=1.0` log-frequency group-delay / phase-dispersion ripple
/// across active cascaded allpass stages at `freq_hz` for `FxSlotVisualizer`.
pub fn evaluate_diopser_ripple(p: [f32; 4], stages_idx: usize, stack: u8, freq_hz: f32) -> f32 {
    let num_stages = diopser_stage_count(stages_idx);
    let center_hz = diopser_freq_hz(p[0]);
    let q = diopser_res_to_q(p[1]) as f32;
    let spread = p[2].clamp(0.0, 1.0);
    let mix = p[3].clamp(0.0, 1.0);
    let stack_mult = (stack as f32).clamp(1.0, 4.0);
    let f = freq_hz.clamp(20.0, 20_000.0);

    let mut total_disp = 0.0_f32;
    for i in 0..num_stages {
        let fi = diopser_stage_freq_hz(center_hz, spread, i, num_stages, 18_000.0);
        let r = (f / fi).max(1e-4);
        let r2 = r * r;
        let one_minus_r2 = 1.0 - r2;
        let denom = (q * one_minus_r2) * (q * one_minus_r2) + r2;
        let stage_disp = (2.0 * q * r * (1.0 + r2)) / denom.max(1e-5);
        total_disp += stage_disp;
    }

    // Soft logarithmic compression so both 8-stage gentle shifts and 128x4 laser zaps stay readable.
    let weighted = total_disp * stack_mult * (0.25 + 0.75 * mix);
    ((weighted * 0.14).ln_1p() / (32.0_f32).ln_1p()).clamp(0.0, 1.0)
}

use super::flush_denormal;

/// Pre-allocated stereo cascaded 2nd-order allpass bank (`MAX_DIOPSER_STAGES = 128`).
pub(crate) struct DiopserStereo {
    stages_l: Box<[Biquad; MAX_DIOPSER_STAGES]>,
    stages_r: Box<[Biquad; MAX_DIOPSER_STAGES]>,
    coeffs: Box<[BiquadCoeffs; MAX_DIOPSER_STAGES]>,
    sample_rate: f32,
    last_freq_norm: f32,
    last_res_norm: f32,
    last_spread_norm: f32,
    last_stages_idx: usize,
    last_sr: f32,
    dirty: bool,
    idle_samples: u32,
}

impl DiopserStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            stages_l: vec![Biquad::default(); MAX_DIOPSER_STAGES]
                .into_boxed_slice()
                .try_into()
                .expect("MAX_DIOPSER_STAGES mismatch"),
            stages_r: vec![Biquad::default(); MAX_DIOPSER_STAGES]
                .into_boxed_slice()
                .try_into()
                .expect("MAX_DIOPSER_STAGES mismatch"),
            coeffs: vec![BiquadCoeffs::default(); MAX_DIOPSER_STAGES]
                .into_boxed_slice()
                .try_into()
                .expect("MAX_DIOPSER_STAGES mismatch"),
            sample_rate: sr,
            last_freq_norm: -1.0,
            last_res_norm: -1.0,
            last_spread_norm: -1.0,
            last_stages_idx: usize::MAX,
            last_sr: 0.0,
            dirty: false,
            idle_samples: 4096,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        self.last_sr = 0.0;
    }

    pub(crate) fn reset(&mut self) {
        if self.dirty {
            for i in 0..MAX_DIOPSER_STAGES {
                self.stages_l[i].reset();
                self.stages_r[i].reset();
            }
            self.dirty = false;
        }
        self.idle_samples = 4096;
    }

    #[inline]
    fn update_coeffs(&mut self, freq_n: f32, res_n: f32, spread_n: f32, stages_idx: usize) {
        if (freq_n - self.last_freq_norm).abs() < 1e-5
            && (res_n - self.last_res_norm).abs() < 1e-5
            && (spread_n - self.last_spread_norm).abs() < 1e-5
            && stages_idx == self.last_stages_idx
            && (self.sample_rate - self.last_sr).abs() < 0.5
        {
            return;
        }

        let num_stages = diopser_stage_count(stages_idx);
        let center_hz = diopser_freq_hz(freq_n);
        let q = diopser_res_to_q(res_n);
        let max_hz = (self.sample_rate * 0.44).min(18_000.0);
        let sr64 = self.sample_rate as f64;

        for i in 0..num_stages {
            let hz = diopser_stage_freq_hz(center_hz, spread_n, i, num_stages, max_hz) as f64;
            let c = BiquadCoeffs::all_pass(sr64, hz, q);
            self.coeffs[i] = c;
            self.stages_l[i].set_coeffs(c);
            self.stages_r[i].set_coeffs(c);
        }

        self.last_freq_norm = freq_n;
        self.last_res_norm = res_n;
        self.last_spread_norm = spread_n;
        self.last_stages_idx = stages_idx;
        self.last_sr = self.sample_rate;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        stages_idx: usize,
    ) -> (f32, f32, f32) {
        let silent_in = in_l.abs() < 1e-8 && in_r.abs() < 1e-8;
        if silent_in {
            self.idle_samples = self.idle_samples.saturating_add(1);
            if !self.dirty {
                return (0.0, 0.0, 0.0);
            }
        } else {
            self.idle_samples = 0;
        }

        let freq_n = p[0].clamp(0.0, 1.0);
        let res_n = p[1].clamp(0.0, 1.0);
        let spread_n = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);
        let clamped_idx = stages_idx.min(DIOPSER_STAGE_COUNTS.len() - 1);
        let num_stages = diopser_stage_count(clamped_idx);

        self.update_coeffs(freq_n, res_n, spread_n, clamped_idx);
        self.dirty = true;

        let mut wl = in_l;
        let mut wr = in_r;
        for i in 0..num_stages {
            wl = self.stages_l[i].process(wl);
            wr = self.stages_r[i].process(wr);
        }
        wl = flush_denormal(wl);
        wr = flush_denormal(wr);

        // Once input is silent for a couple seconds and high-Q tail decays below noise floor, sleep cleanly.
        let tail_timeout = (self.sample_rate * 2.5) as u32;
        if silent_in && self.idle_samples > tail_timeout && wl.abs() < 1e-7 && wr.abs() < 1e-7 {
            self.reset();
            return (0.0, 0.0, 0.0);
        }

        let out_l = flush_denormal(in_l + (wl - in_l) * mix);
        let out_r = flush_denormal(in_r + (wr - in_r) * mix);
        let dispersion_diff = ((wl - in_l).abs().max((wr - in_r).abs()) * 0.5).clamp(0.0, 1.0);
        let act = (dispersion_diff * mix).clamp(0.0, 1.0);
        (out_l, out_r, act)
    }
}
