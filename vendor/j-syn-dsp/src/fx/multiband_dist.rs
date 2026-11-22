//! 3-band phase-aligned (`LR4` + allpass) stereo Multiband Distortion processor.

use crate::oversample::Biquad;
use crate::waveshaper::DistortionMode;

use super::dynamics::{DistortionCore, Lr4BandFilter};
use super::{butterworth_allpass_coeffs, flush_denormal, track_idle};

pub const MB_BAND_LEVEL_DEFAULT_NORM: f32 = 24.0 / 36.0; // 0.0 dB in [-24, +12] dB
pub const MB_XOVER_LOW_DEFAULT_NORM: f32 = 0.5; // 200 Hz in [40, 1000] Hz
pub const MB_XOVER_HIGH_DEFAULT_NORM: f32 = 0.464_385_63; // ~2500 Hz in [500, 16000] Hz

/// Map normalized `[0..1]` -> low crossover frequency (`40 .. 1000 Hz`, log).
#[inline]
pub fn mb_xover_low_hz(norm: f32) -> f32 {
    40.0 * 25.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Inverse mapping of low crossover Hz -> normalized `[0..1]`.
#[inline]
pub fn mb_xover_low_norm(hz: f32) -> f32 {
    (hz.clamp(40.0, 1000.0) / 40.0).ln() / 25.0_f32.ln()
}

/// Map normalized `[0..1]` -> high crossover frequency (`500 .. 16000 Hz`, log).
#[inline]
pub fn mb_xover_high_hz(norm: f32) -> f32 {
    500.0 * 32.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Inverse mapping of high crossover Hz -> normalized `[0..1]`.
#[inline]
pub fn mb_xover_high_norm(hz: f32) -> f32 {
    (hz.clamp(500.0, 16_000.0) / 500.0).ln() / 32.0_f32.ln()
}

/// Map normalized `[0..1]` -> global output gain (`-18 .. +18 dB`).
#[inline]
pub fn mb_out_gain_db(norm: f32) -> f32 {
    -18.0 + norm.clamp(0.0, 1.0) * 36.0
}

/// Map normalized `[0..1]` -> per-band output level (`-24 .. +12 dB`).
#[inline]
pub fn mb_band_level_db(norm: f32) -> f32 {
    -24.0 + norm.clamp(0.0, 1.0) * 36.0
}

/// Compute ordered `(low_hz, high_hz)` ensuring `high_hz >= low_hz + 40 Hz`.
#[inline]
pub fn mb_ordered_xover_hz(p0: f32, p1: f32) -> (f32, f32) {
    let low = mb_xover_low_hz(p0);
    let high = mb_xover_high_hz(p1).max(low + 40.0);
    (low, high)
}

/// Evaluates ideal `LR4` band magnitude gain (`0.0..=1.0` linear) at `freq_hz` for scope display.
pub fn mb_band_magnitude_at_hz(band: usize, low_hz: f32, high_hz: f32, freq_hz: f32) -> f32 {
    let f = freq_hz.max(1.0);
    let r_lo4 = (f / low_hz.max(1.0)).powi(4);
    let r_hi4 = (f / high_hz.max(1.0)).powi(4);
    let lp_lo = 1.0 / (1.0 + r_lo4);
    let hp_lo = r_lo4 / (1.0 + r_lo4);
    let lp_hi = 1.0 / (1.0 + r_hi4);
    let hp_hi = r_hi4 / (1.0 + r_hi4);
    match band {
        0 => lp_lo,
        1 => hp_lo * lp_hi,
        _ => hp_lo * hp_hi,
    }
}

/// Parameters for one band (`0 = Low`, `1 = Mid`, `2 = High`) of `MultiBandDistStereo`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MbDistBandParams {
    pub mode: DistortionMode,
    pub enabled: bool,
    pub solo: bool,
    pub drive: f32,
    pub bias: f32,
    pub level: f32,
    pub mix: f32,
}

impl Default for MbDistBandParams {
    fn default() -> Self {
        Self {
            mode: DistortionMode::Tube,
            enabled: true,
            solo: false,
            drive: 0.30,
            bias: 0.50,
            level: MB_BAND_LEVEL_DEFAULT_NORM,
            mix: 1.00,
        }
    }
}

/// 3-band phase-aligned `LR4` Multiband Distortion processor (`LOW` / `MID` / `HIGH`).
pub(crate) struct MultiBandDistStereo {
    low_split: [Lr4BandFilter; 2],
    high_split: [Lr4BandFilter; 2],
    low_ap: [Biquad; 2],
    cores: [DistortionCore; 3],
    last_low_hz: f32,
    last_high_hz: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl MultiBandDistStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            low_split: [Lr4BandFilter::default(); 2],
            high_split: [Lr4BandFilter::default(); 2],
            low_ap: [Biquad::default(); 2],
            cores: [
                DistortionCore::new(sample_rate),
                DistortionCore::new(sample_rate),
                DistortionCore::new(sample_rate),
            ],
            last_low_hz: -1.0,
            last_high_hz: -1.0,
            sample_rate: 44_100.0,
            idle_samples: 1024,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        for core in &mut self.cores {
            core.set_sample_rate(self.sample_rate);
        }
        self.last_low_hz = -1.0;
        self.last_high_hz = -1.0;
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..2 {
            self.low_split[ch].reset();
            self.high_split[ch].reset();
            self.low_ap[ch].reset();
        }
        for core in &mut self.cores {
            core.reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    fn update_crossovers(&mut self, low_hz: f32, high_hz: f32) {
        if (low_hz - self.last_low_hz).abs() > 0.05 || (high_hz - self.last_high_hz).abs() > 0.05
        {
            let sr = self.sample_rate as f64;
            let ap_high = butterworth_allpass_coeffs(sr, high_hz as f64);
            for ch in 0..2 {
                self.low_split[ch].set_freq(sr, low_hz as f64);
                self.high_split[ch].set_freq(sr, high_hz as f64);
                self.low_ap[ch].set_coeffs(ap_high);
            }
            self.last_low_hz = low_hz;
            self.last_high_hz = high_hz;
        }
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        bands: &[MbDistBandParams; 3],
        p: [f32; 4],
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 256 {
            return (0.0, 0.0, 0.0);
        }

        let (low_hz, high_hz) = mb_ordered_xover_hz(p[0], p[1]);
        let out_gain = 10.0_f32.powf(mb_out_gain_db(p[2]) * 0.05);
        let global_mix = p[3].clamp(0.0, 1.0);

        self.update_crossovers(low_hz, high_hz);

        // Phase-coherent LR4 3-band split: low band gets upper-xover allpass compensation
        let (low_raw_l, rest_l) = self.low_split[0].split(in_l);
        let (low_raw_r, rest_r) = self.low_split[1].split(in_r);
        let low_l = self.low_ap[0].process(low_raw_l);
        let low_r = self.low_ap[1].process(low_raw_r);
        let (mid_l, high_l) = self.high_split[0].split(rest_l);
        let (mid_r, high_r) = self.high_split[1].split(rest_r);

        if global_mix <= 1e-5 {
            return (in_l, in_r, 0.0);
        }

        let band_in = [(low_l, low_r), (mid_l, mid_r), (high_l, high_r)];
        let any_solo = bands.iter().any(|b| b.solo);

        let mut wet_l = 0.0_f32;
        let mut wet_r = 0.0_f32;
        let mut act_sum = 0.0_f32;

        for (b_idx, (band, (bl, br))) in bands.iter().zip(band_in.iter().copied()).enumerate() {
            if any_solo && !band.solo {
                continue;
            }
            if !band.enabled {
                // Bypassed band passes dry signal cleanly at unity
                wet_l += bl;
                wet_r += br;
                continue;
            }

            let drive = band.drive.clamp(0.0, 1.0);
            let bias = (band.bias.clamp(0.0, 1.0) * 2.0) - 1.0;
            let level = 10.0_f32.powf(mb_band_level_db(band.level) * 0.05);
            let mix = band.mix.clamp(0.0, 1.0);

            let (sl, sr) = if mix <= 1e-5 {
                (bl, br)
            } else {
                let (wl, wr) = self.cores[b_idx].process_wet(bl, br, band.mode, bias, drive);
                (bl + (wl - bl) * mix, br + (wr - br) * mix)
            };

            wet_l += sl * level;
            wet_r += sr * level;
            act_sum += drive * mix;
        }

        wet_l *= out_gain;
        wet_r *= out_gain;

        let out_l = in_l + (wet_l - in_l) * global_mix;
        let out_r = in_r + (wet_r - in_r) * global_mix;
        let activity = (act_sum * (1.0 / 3.0) * global_mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}
