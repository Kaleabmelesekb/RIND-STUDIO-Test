//! Dynamics and waveshaping FX processors (`OTT`, `Distortion`, `Clipper`).
//! Zero audio-thread heap allocations; uses pre-allocated biquad crossovers & 4x oversamplers.

use crate::adaa::TanhAdaa2;
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};
use crate::waveshaper::{shape_sample, DistortionMode};

use super::{
    butterworth_allpass_coeffs, butterworth_hp_coeffs, butterworth_lp_coeffs, flush_denormal,
    track_idle,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Lr4BandFilter {
    lp1: Biquad,
    lp2: Biquad,
    hp1: Biquad,
    hp2: Biquad,
}

impl Lr4BandFilter {
    pub(crate) fn set_freq(&mut self, sample_rate: f64, freq_hz: f64) {
        let lp = butterworth_lp_coeffs(sample_rate, freq_hz);
        let hp = butterworth_hp_coeffs(sample_rate, freq_hz);
        self.lp1.set_coeffs(lp);
        self.lp2.set_coeffs(lp);
        self.hp1.set_coeffs(hp);
        self.hp2.set_coeffs(hp);
    }

    pub(crate) fn reset(&mut self) {
        self.lp1.reset();
        self.lp2.reset();
        self.hp1.reset();
        self.hp2.reset();
    }

    #[inline]
    pub(crate) fn split(&mut self, x: f32) -> (f32, f32) {
        let low = self.lp2.process(self.lp1.process(x));
        let high = self.hp2.process(self.hp1.process(x));
        (low, high)
    }
}

#[derive(Debug, Clone, Copy)]
struct OttBandComp {
    env: f32,
    up_thresh_db: f32,
    down_thresh_db: f32,
}

impl OttBandComp {
    const fn new(up_thresh_db: f32, down_thresh_db: f32) -> Self {
        Self {
            env: 0.0,
            up_thresh_db,
            down_thresh_db,
        }
    }

    fn reset(&mut self) {
        self.env = 0.0;
    }

    #[inline]
    fn compute_gain(&mut self, abs_sample: f32, atk_coeff: f32, rel_coeff: f32) -> (f32, f32) {
        let coeff = if abs_sample > self.env {
            atk_coeff
        } else {
            rel_coeff
        };
        self.env += coeff * (abs_sample - self.env);
        self.env = flush_denormal(self.env);

        if self.env <= 1e-5 {
            return (1.0, 0.0);
        }

        let env_db = 20.0 * self.env.log10();
        let gate = ((env_db + 78.0) / 12.0).clamp(0.0, 1.0);

        let boost_db = if env_db < self.up_thresh_db {
            ((self.up_thresh_db - env_db) * 0.72).min(18.0) * gate
        } else {
            0.0
        };

        let cut_db = if env_db > self.down_thresh_db {
            (env_db - self.down_thresh_db) * 0.85
        } else {
            0.0
        };

        let gain_db = boost_db - cut_db;
        let gain_lin = 10.0_f32.powf(gain_db * 0.05);
        let activity = ((boost_db + cut_db) / 18.0).clamp(0.0, 1.0);
        (gain_lin, activity)
    }
}

/// 3-band OTT compressor (88.3 Hz & 2500 Hz LR4 phase-coherent crossover).
pub(crate) struct OttStereo {
    low_split: [Lr4BandFilter; 2],
    high_split: [Lr4BandFilter; 2],
    low_ap: [Biquad; 2],
    bands: [OttBandComp; 3],
    sample_rate: f32,
    idle_samples: u32,
}

impl OttStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            low_split: [Lr4BandFilter::default(); 2],
            high_split: [Lr4BandFilter::default(); 2],
            low_ap: [Biquad::default(); 2],
            bands: [
                OttBandComp::new(-40.0, -24.0),
                OttBandComp::new(-38.0, -22.0),
                OttBandComp::new(-38.0, -22.0),
            ],
            sample_rate: 44_100.0,
            idle_samples: 1024,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0) as f64;
        self.sample_rate = sr as f32;
        let ap_high = butterworth_allpass_coeffs(sr, 2500.0);
        for ch in 0..2 {
            self.low_split[ch].set_freq(sr, 88.3);
            self.high_split[ch].set_freq(sr, 2500.0);
            self.low_ap[ch].set_coeffs(ap_high);
        }
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..2 {
            self.low_split[ch].reset();
            self.high_split[ch].reset();
            self.low_ap[ch].reset();
        }
        for band in &mut self.bands {
            band.reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r)
            && self.idle_samples > 256
            && self.bands[0].env < 1e-6
            && self.bands[1].env < 1e-6
            && self.bands[2].env < 1e-6
        {
            return (0.0, 0.0, 0.0);
        }

        let depth = p[0].clamp(0.0, 1.0);
        let time_scale = 0.25 * 16.0_f32.powf(p[1].clamp(0.0, 1.0));
        let in_db = -18.0 + p[2].clamp(0.0, 1.0) * 36.0;
        let out_db = -18.0 + p[3].clamp(0.0, 1.0) * 36.0;
        let in_gain = 10.0_f32.powf(in_db * 0.05);
        let out_gain = 10.0_f32.powf(out_db * 0.05);

        let x_l = in_l * in_gain;
        let x_r = in_r * in_gain;

        // Phase-coherent LR4 3-band split: low passed through 2500 Hz allpass matches mid+high group delay
        let (low_raw_l, rest_l) = self.low_split[0].split(x_l);
        let (low_raw_r, rest_r) = self.low_split[1].split(x_r);
        let low_l = self.low_ap[0].process(low_raw_l);
        let low_r = self.low_ap[1].process(low_raw_r);
        let (mid_l, high_l) = self.high_split[0].split(rest_l);
        let (mid_r, high_r) = self.high_split[1].split(rest_r);

        let sr = self.sample_rate.max(1_000.0);
        let atk_s = 0.012 * time_scale;
        let rel_s = 0.140 * time_scale;
        let atk_c = 1.0 - (-1.0 / (atk_s * sr)).exp();
        let rel_c = 1.0 - (-1.0 / (rel_s * sr)).exp();

        let (g0, a0) = self.bands[0].compute_gain(low_l.abs().max(low_r.abs()), atk_c, rel_c);
        let (g1, a1) = self.bands[1].compute_gain(mid_l.abs().max(mid_r.abs()), atk_c, rel_c);
        let (g2, a2) = self.bands[2].compute_gain(high_l.abs().max(high_r.abs()), atk_c, rel_c);

        let wet_l = (low_l * g0 + mid_l * g1 + high_l * g2) * out_gain;
        let wet_r = (low_r * g0 + mid_r * g1 + high_r * g2) * out_gain;
        let dry_l = (low_l + mid_l + high_l) * out_gain;
        let dry_r = (low_r + mid_r + high_r) * out_gain;

        let l = dry_l + (wet_l - dry_l) * depth;
        let r = dry_r + (wet_r - dry_r) * depth;
        let activity = ((a0 + a1 + a2) * (1.0 / 3.0) * depth).clamp(0.0, 1.0);
        (flush_denormal(l), flush_denormal(r), activity)
    }
}

/// Shared 4x oversampled stereo waveshaper core reused by `DistortionStereo` and `MultiBandDistStereo`.
pub(crate) struct DistortionCore {
    oversamplers: [Oversampler4x; 2],
    mech_low_shelf: [Biquad; 2],
    mech_high_shelf: [Biquad; 2],
    classic_hp_x: [f32; 2],
    classic_hp_y: [f32; 2],
    classic_lp: [f32; 2],
    crush_phase: f32,
    hold_l: f32,
    hold_r: f32,
    sample_rate: f32,
}

impl DistortionCore {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            oversamplers: [Oversampler4x::new(); 2],
            mech_low_shelf: [Biquad::default(); 2],
            mech_high_shelf: [Biquad::default(); 2],
            classic_hp_x: [0.0; 2],
            classic_hp_y: [0.0; 2],
            classic_lp: [0.0; 2],
            crush_phase: 1.0,
            hold_l: 0.0,
            hold_r: 0.0,
            sample_rate: 44_100.0,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        let os_rate = self.sample_rate as f64 * 4.0;
        let low_coeffs = BiquadCoeffs::low_shelf_linear(os_rate, 200.0, 0.7, 1.8);
        let high_coeffs = BiquadCoeffs::high_shelf_linear(os_rate, 4000.0, 0.7, 2.0);
        for ch in 0..2 {
            self.mech_low_shelf[ch].set_coeffs(low_coeffs);
            self.mech_high_shelf[ch].set_coeffs(high_coeffs);
        }
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.mech_low_shelf[ch].reset();
            self.mech_high_shelf[ch].reset();
        }
        self.classic_hp_x = [0.0; 2];
        self.classic_hp_y = [0.0; 2];
        self.classic_lp = [0.0; 2];
        self.crush_phase = 1.0;
        self.hold_l = 0.0;
        self.hold_r = 0.0;
    }

    #[inline]
    pub(crate) fn process_wet(
        &mut self,
        in_l: f32,
        in_r: f32,
        mode: DistortionMode,
        bias: f32,
        drive: f32,
    ) -> (f32, f32) {
        if mode == DistortionMode::Crush {
            let step = 1.0 / (1.0 + drive * drive * 31.0);
            let levels = 2.0_f32.powf(11.0 - drive * 8.5);
            let bo = bias * 0.2;
            let bq = (bo.clamp(-1.5, 1.5) * levels).round() / levels;
            self.crush_phase += step;
            if self.crush_phase >= 1.0 {
                self.crush_phase = self.crush_phase.fract();
                self.hold_l = ((in_l + bo).clamp(-1.5, 1.5) * levels).round() / levels - bq;
                self.hold_r = ((in_r + bo).clamp(-1.5, 1.5) * levels).round() / levels - bq;
            }
            (self.hold_l, self.hold_r)
        } else {
            let mut ch_out = [0.0_f32; 2];
            let inputs = [in_l, in_r];
            for ch in 0..2 {
                let low_shelf = &mut self.mech_low_shelf[ch];
                let high_shelf = &mut self.mech_high_shelf[ch];
                ch_out[ch] = self.oversamplers[ch].process_sample(inputs[ch], |x| match mode {
                    DistortionMode::Mech => {
                        let m = high_shelf.process(low_shelf.process(x));
                        shape_sample(DistortionMode::Mech, m, drive, bias)
                    }
                    _ => shape_sample(mode, x, drive, bias),
                });
            }
            if mode == DistortionMode::ClassicA {
                let hp_r = (-std::f32::consts::TAU * 35.0 / self.sample_rate)
                    .exp()
                    .clamp(0.9, 0.9999);
                let lp_a = (1.0 - (-std::f32::consts::TAU * 18_000.0 / self.sample_rate).exp())
                    .clamp(0.05, 1.0);
                for ch in 0..2 {
                    let x = ch_out[ch];
                    let hp = x - self.classic_hp_x[ch] + hp_r * self.classic_hp_y[ch];
                    self.classic_hp_x[ch] = x;
                    self.classic_hp_y[ch] = hp;
                    self.classic_lp[ch] += lp_a * (hp - self.classic_lp[ch]);
                    ch_out[ch] = self.classic_lp[ch];
                }
            }
            (ch_out[0], ch_out[1])
        }
    }
}

/// 4x oversampled stereo master distortion (`Tube`, `Mech`, `Sine`, `Crush`, `Classic A/B/C`,
/// `Diode A/B`, `Linear/Sine Fold`, `Hard Clip`, `Half/Full Rect`).
pub(crate) struct DistortionStereo {
    core: DistortionCore,
    tone_lp_l: f32,
    tone_lp_r: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl DistortionStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            core: DistortionCore::new(sample_rate),
            tone_lp_l: 0.0,
            tone_lp_r: 0.0,
            sample_rate: 44_100.0,
            idle_samples: 1024,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        self.core.set_sample_rate(sample_rate);
    }

    pub(crate) fn reset(&mut self) {
        self.core.reset();
        self.tone_lp_l = 0.0;
        self.tone_lp_r = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        mode: DistortionMode,
        p: [f32; 4],
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 128 {
            return (0.0, 0.0, 0.0);
        }

        let bias = (p[0].clamp(0.0, 1.0) * 2.0) - 1.0;
        let drive = p[1].clamp(0.0, 1.0);
        let tone = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        if mix <= 1e-5 {
            return (in_l, in_r, 0.0);
        }

        let (mut wl, mut wr) = self.core.process_wet(in_l, in_r, mode, bias, drive);

        if tone < 0.495 {
            let t_norm = (0.5 - tone) * 2.0;
            let fc = 18_000.0 * (800.0_f32 / 18_000.0).powf(t_norm);
            let alpha = (2.0 * std::f32::consts::PI * fc / self.sample_rate).clamp(0.02, 0.99);
            self.tone_lp_l += alpha * (wl - self.tone_lp_l);
            self.tone_lp_r += alpha * (wr - self.tone_lp_r);
            wl = self.tone_lp_l;
            wr = self.tone_lp_r;
        } else {
            let alpha = (2.0 * std::f32::consts::PI * 2200.0 / self.sample_rate).clamp(0.02, 0.99);
            self.tone_lp_l += alpha * (wl - self.tone_lp_l);
            self.tone_lp_r += alpha * (wr - self.tone_lp_r);
            let hi_boost = ((tone - 0.5) * 2.0).max(0.0) * 0.85;
            wl += (wl - self.tone_lp_l) * hi_boost;
            wr += (wr - self.tone_lp_r) * hi_boost;
        }

        let out_l = in_l + (wl - in_l) * mix;
        let out_r = in_r + (wr - in_r) * mix;
        let activity = (drive * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

/// 4x oversampled J-SAT Clipper (`TanhAdaa2` soft-knee + digital hard ceiling).
pub(crate) struct JSatClipperStereo {
    oversamplers: [Oversampler4x; 2],
    adaa: [TanhAdaa2; 2],
    idle_samples: u32,
}

impl JSatClipperStereo {
    pub(crate) fn new() -> Self {
        Self {
            oversamplers: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            idle_samples: 1024,
        }
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.adaa[ch].reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 128 {
            return (0.0, 0.0, 0.0);
        }

        let drive_db = p[0].clamp(0.0, 1.0) * 24.0;
        let ceil_db = -18.0 + p[1].clamp(0.0, 1.0) * 18.0;
        let knee = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        let drive_lin = 10.0_f32.powf(drive_db * 0.05);
        let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).max(0.02);
        let inv_ceil = (1.0 / ceil_lin) as f64;

        let driven_l = in_l * drive_lin;
        let driven_r = in_r * drive_lin;
        let driven_peak = driven_l.abs().max(driven_r.abs());

        let mut ch_out = [0.0_f32; 2];
        let inputs = [driven_l, driven_r];
        for ch in 0..2 {
            let adaa = &mut self.adaa[ch];
            ch_out[ch] = self.oversamplers[ch].process_sample(inputs[ch], |x| {
                let hard = x.clamp(-ceil_lin, ceil_lin);
                if knee <= 1e-4 {
                    hard
                } else {
                    let soft = (adaa.process((x as f64) * inv_ceil) * ceil_lin as f64) as f32;
                    hard + (soft - hard) * knee
                }
            });
        }

        let post_peak = ch_out[0].abs().max(ch_out[1].abs());
        let gr_activity = if driven_peak > 1e-5 {
            ((driven_peak - post_peak).max(0.0) / driven_peak).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let out_l = in_l + (ch_out[0] - in_l) * mix;
        let out_r = in_r + (ch_out[1] - in_r) * mix;
        (flush_denormal(out_l), flush_denormal(out_r), gr_activity * mix)
    }
}
