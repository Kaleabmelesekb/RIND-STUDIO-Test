//! Time-based, modulation, and shared wet-EQ FX processors (`Chorus`, `Phaser`, `Delay`, `Reverb`).
//! Zero audio-thread heap allocations; all delay lines are pre-allocated once on `new()`.

use std::f64::consts::FRAC_1_SQRT_2;

use crate::filter::fast_tanh;
use crate::oversample::{Biquad, BiquadCoeffs};

const CHORUS_BUF_LEN: usize = 4096;
const MAX_FLANGE_SAMPLES: usize = 4096;
const MAX_DELAY_SAMPLES: usize = 96_000;
const REVERB_COMB_LEN: usize = 4096;
const REVERB_AP_LEN: usize = 1024;
const REVERB_PREDELAY_LEN: usize = 32_768;
const PHASER_STAGES: usize = 6;

pub const DELAY_SYNC_LABELS: [&str; 8] = ["1/16", "1/8T", "1/8", "1/8D", "1/4", "1/4D", "1/2", "1/1"];
const DELAY_SYNC_BEATS: [f32; 8] = [0.25, 1.0 / 3.0, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0];

const COMB_BASE: [usize; 4] = [1116, 1277, 1422, 1617];
const AP_BASE: [usize; 2] = [556, 441];
const STEREO_SPREAD: usize = 29;

use super::{flush_denormal, track_idle};

/// Maps normalized `0.0..=1.0` to `20 Hz .. 2000 Hz`.
#[inline]
pub fn fx_low_cut_hz(norm: f32) -> f32 {
    20.0 * 100.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Maps normalized `0.0..=1.0` to `500 Hz .. 20_000 Hz`.
#[inline]
pub fn fx_high_cut_hz(norm: f32) -> f32 {
    500.0 * 40.0_f32.powf(norm.clamp(0.0, 1.0))
}

#[inline]
pub fn apply_stereo_width(l: f32, r: f32, width_norm: f32) -> (f32, f32) {
    let side_scale = (width_norm.clamp(0.0, 1.0)) * 2.0;
    let mid = 0.5 * (l + r);
    let side = 0.5 * (l - r) * side_scale;
    (mid + side, mid - side)
}

/// Maps normalized `0.0..=1.0` to `0.02 Hz .. 15.0 Hz` Flanger LFO rate (log).
#[inline]
pub fn flanger_rate_hz(norm: f32) -> f32 {
    0.02 * 750.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Maps normalized `0.0..=1.0` to `0.1 ms .. 15.0 ms` Flanger center delay (log).
#[inline]
pub fn flanger_delay_ms(norm: f32) -> f32 {
    0.1 * 150.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Maps normalized `0.0..=1.0` to bipolar `-99% .. +99%` Flanger feedback (`0.50` = `0%`).
#[inline]
pub fn flanger_feedback(norm: f32) -> f32 {
    (norm.clamp(0.0, 1.0) * 2.0 - 1.0) * 0.99
}

/// Maps normalized `0.0..=1.0` to `0° .. 180°` stereo L/R LFO phase offset.
#[inline]
pub fn flanger_phase_deg(norm: f32) -> f32 {
    norm.clamp(0.0, 1.0) * 180.0
}

#[inline]
fn read_hermite(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
    let len = buf.len();
    let d = delay_samples.clamp(1.0, (len - 3) as f32);
    let d_int = d as usize;
    let t = d - d_int as f32;
    let y_m1 = buf[(write_pos + len - (d_int - 1)) % len];
    let y0 = buf[(write_pos + len - d_int) % len];
    let y1 = buf[(write_pos + len - (d_int + 1)) % len];
    let y2 = buf[(write_pos + len - (d_int + 2)) % len];
    let c0 = y0;
    let c1 = 0.5 * (y1 - y_m1);
    let c2 = y_m1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
    let c3 = 0.5 * (y2 - y_m1) + 1.5 * (y0 - y1);
    ((c3 * t + c2) * t + c1) * t + c0
}

#[inline]
pub(crate) fn flanger_comb_mag(delay_s: f32, fb: f32, freq_hz: f32) -> f32 {
    let theta = -std::f32::consts::TAU * freq_hz * delay_s;
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    // H(w) = 0.5 + 0.5 * z^-d / (1 - fb * z^-d)
    let denom_re = 1.0 - fb * cos_t;
    let denom_im = -fb * sin_t;
    let denom_sq = (denom_re * denom_re + denom_im * denom_im).max(1e-6);
    let tap_re = 0.5 * (cos_t * denom_re + sin_t * denom_im) / denom_sq;
    let tap_im = 0.5 * (sin_t * denom_re - cos_t * denom_im) / denom_sq;
    let re = 0.5 + tap_re;
    let im = tap_im;
    (re * re + im * im).sqrt().clamp(0.0, 4.0)
}

/// Evaluates instantaneous Flanger L/R comb magnitude responses at `freq_hz` for scope display.
pub fn evaluate_flanger_scope_response(
    p: [f32; 4],
    eq: [f32; 4],
    lfo_phase: f32,
    freq_hz: f32,
) -> (f32, f32, f32, f32) {
    let depth = p[1].clamp(0.0, 1.0);
    let center_ms = flanger_delay_ms(p[2]);
    let fb = flanger_feedback(eq[0]);
    let phase_rad = eq[1].clamp(0.0, 1.0) * std::f32::consts::PI;

    let ang_l = lfo_phase * std::f32::consts::TAU;
    let ang_r = ang_l + phase_rad;
    let delay_ms = |ang: f32| (center_ms * 4.0_f32.powf(depth * 0.65 * ang.sin())).clamp(0.08, 19.5);
    let dl = delay_ms(ang_l);
    let dr = delay_ms(ang_r);
    (
        flanger_comb_mag(dl * 0.001, fb, freq_hz),
        flanger_comb_mag(dr * 0.001, fb, freq_hz),
        dl,
        dr,
    )
}

#[inline]
fn read_linear(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
    let len = buf.len();
    let d = delay_samples.clamp(1.0, (len - 2) as f32);
    let d_int = d as usize;
    let frac = d - d_int as f32;
    let idx0 = (write_pos + len - d_int) % len;
    let idx1 = (write_pos + len - (d_int + 1)) % len;
    buf[idx0] + (buf[idx1] - buf[idx0]) * frac
}

/// Shared wet-path HP/LP filter pair (`LOW CUT` + `HIGH CUT`) with coefficient caching.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EqStageStereo {
    hp: [Biquad; 2],
    lp: [Biquad; 2],
    sample_rate: f32,
    last_hp_norm: f32,
    last_lp_norm: f32,
    last_sr: f32,
}

impl EqStageStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        Self {
            hp: [Biquad::default(); 2],
            lp: [Biquad::default(); 2],
            sample_rate: sample_rate.max(1_000.0),
            last_hp_norm: -1.0,
            last_lp_norm: -1.0,
            last_sr: 0.0,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        self.last_sr = 0.0;
    }

    pub(crate) fn reset(&mut self) {
        for ch in 0..2 {
            self.hp[ch].reset();
            self.lp[ch].reset();
        }
    }

    #[inline]
    pub(crate) fn process(&mut self, in_l: f32, in_r: f32, low_cut: f32, high_cut: f32) -> (f32, f32) {
        let hp_n = low_cut.clamp(0.0, 1.0);
        let lp_n = high_cut.clamp(0.0, 1.0);
        if (hp_n - self.last_hp_norm).abs() > 1e-4
            || (lp_n - self.last_lp_norm).abs() > 1e-4
            || (self.sample_rate - self.last_sr).abs() > 0.5
        {
            let sr64 = self.sample_rate as f64;
            let hp_hz = fx_low_cut_hz(hp_n).min(self.sample_rate * 0.44) as f64;
            let lp_hz = fx_high_cut_hz(lp_n).min(self.sample_rate * 0.44) as f64;
            let hp_c = BiquadCoeffs::high_pass(sr64, hp_hz, FRAC_1_SQRT_2);
            let lp_c = BiquadCoeffs::low_pass(sr64, lp_hz, FRAC_1_SQRT_2);
            for ch in 0..2 {
                self.hp[ch].set_coeffs(hp_c);
                self.lp[ch].set_coeffs(lp_c);
            }
            self.last_hp_norm = hp_n;
            self.last_lp_norm = lp_n;
            self.last_sr = self.sample_rate;
        }

        let mut l = in_l;
        let mut r = in_r;
        if hp_n > 0.002 {
            l = self.hp[0].process(l);
            r = self.hp[1].process(r);
        }
        if lp_n < 0.998 {
            l = self.lp[0].process(l);
            r = self.lp[1].process(r);
        }
        (flush_denormal(l), flush_denormal(r))
    }
}

/// Stereo Chorus / Dimension (`MAIN` = `RATE, DEPTH, DIM, MIX`, `EQ` = `LOW CUT, HIGH CUT, SPREAD, FEEDBK`).
pub(crate) struct ChorusStereo {
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    fb_l: f32,
    fb_r: f32,
    eq: EqStageStereo,
    write_pos: usize,
    lfo_phase: f32,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl ChorusStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            buf_l: vec![0.0; CHORUS_BUF_LEN].into_boxed_slice(),
            buf_r: vec![0.0; CHORUS_BUF_LEN].into_boxed_slice(),
            fb_l: 0.0,
            fb_r: 0.0,
            eq: EqStageStereo::new(sr),
            write_pos: 0,
            lfo_phase: 0.0,
            sample_rate: sr,
            dirty: false,
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.eq.set_sample_rate(sr);
    }

    pub(crate) fn reset(&mut self) {
        if self.dirty {
            self.buf_l.fill(0.0);
            self.buf_r.fill(0.0);
            self.dirty = false;
        }
        self.fb_l = 0.0;
        self.fb_r = 0.0;
        self.eq.reset();
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) {
            if !self.dirty {
                return (0.0, 0.0, 0.0);
            }
            if self.idle_samples > (CHORUS_BUF_LEN as u32 + 64) {
                self.reset();
                return (0.0, 0.0, 0.0);
            }
        }

        let rate_hz = 0.05 * 100.0_f32.powf(p[0].clamp(0.0, 1.0));
        let depth = p[1].clamp(0.0, 1.0);
        let dim = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        let low_cut = eq[0].clamp(0.0, 1.0);
        let high_cut = eq[1].clamp(0.0, 1.0);
        let spread = eq[2].clamp(0.0, 1.0);
        let feedback = eq[3].clamp(0.0, 1.0) * 0.75;

        self.dirty = true;
        self.buf_l[self.write_pos] = fast_tanh((in_l + self.fb_l * feedback) * 0.65) * 1.5;
        self.buf_r[self.write_pos] = fast_tanh((in_r + self.fb_r * feedback) * 0.65) * 1.5;

        self.lfo_phase = (self.lfo_phase + rate_hz / self.sample_rate).fract();
        let angle_l = self.lfo_phase * std::f32::consts::TAU;
        let angle_r = angle_l + dim * std::f32::consts::PI;

        let sr_ms = self.sample_rate * 0.001;
        let center_ms = 6.5 + dim * 6.5;
        let swing_ms = depth * 4.5;
        let d_l = (center_ms + swing_ms * angle_l.sin()) * sr_ms;
        let d_r = (center_ms + 1.2 * dim + swing_ms * angle_r.sin()) * sr_ms;

        let wet_raw_l = read_linear(&self.buf_l, self.write_pos, d_l);
        let wet_raw_r = read_linear(&self.buf_r, self.write_pos, d_r);
        self.fb_l = flush_denormal(wet_raw_l);
        self.fb_r = flush_denormal(wet_raw_r);

        // Wet EQ filtering followed by Dimension + EQ Stereo Spread M/S widening
        let (eq_l, eq_r) = self.eq.process(wet_raw_l, wet_raw_r, low_cut, high_cut);
        let mid = 0.5 * (eq_l + eq_r);
        let side_gain = (0.35 + dim * 0.65) * (spread * 2.0);
        let side = 0.5 * (eq_l - eq_r) * side_gain;
        let wet_l = mid + side;
        let wet_r = mid - side;

        self.write_pos = (self.write_pos + 1) % CHORUS_BUF_LEN;
        let out_l = in_l + (wet_l - in_l) * mix;
        let out_r = in_r + (wet_r - in_r) * mix;
        let activity = (0.5 * (angle_l.sin().abs() + angle_r.sin().abs()) * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

/// 6-stage stereo all-pass Phaser with quadrature LFO sweep and soft-limited resonant feedback.
pub(crate) struct PhaserStereo {
    ap_z: [[f32; PHASER_STAGES]; 2],
    fb_state: [f32; 2],
    lfo_phase: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl PhaserStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        Self {
            ap_z: [[0.0; PHASER_STAGES]; 2],
            fb_state: [0.0; 2],
            lfo_phase: 0.0,
            sample_rate: sample_rate.max(1_000.0),
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    pub(crate) fn reset(&mut self) {
        self.ap_z = [[0.0; PHASER_STAGES]; 2];
        self.fb_state = [0.0; 2];
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    fn phase_to_allpass_coeff(&self, norm: f32) -> f32 {
        let fc = (180.0 * 25.5_f32.powf(norm.clamp(0.0, 1.0))).min(self.sample_rate * 0.42);
        let t = (std::f32::consts::PI * fc / self.sample_rate).clamp(0.005, 1.4).tan();
        (t - 1.0) / (t + 1.0)
    }

    #[inline]
    pub(crate) fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 4096 {
            self.ap_z = [[0.0; PHASER_STAGES]; 2];
            self.fb_state = [0.0; 2];
            return (0.0, 0.0, 0.0);
        }

        let rate_hz = 0.05 * 120.0_f32.powf(p[0].clamp(0.0, 1.0));
        let depth = p[1].clamp(0.0, 1.0);
        let fb_gain = p[2].clamp(0.0, 1.0) * 0.88;
        let mix = p[3].clamp(0.0, 1.0);

        self.lfo_phase = (self.lfo_phase + rate_hz / self.sample_rate).fract();
        let lfo_l = 0.5 + 0.5 * (self.lfo_phase * std::f32::consts::TAU).sin();
        let lfo_r = 0.5
            + 0.5 * (self.lfo_phase * std::f32::consts::TAU + std::f32::consts::FRAC_PI_2).sin();

        let sweep_l = 0.5 + (lfo_l - 0.5) * depth;
        let sweep_r = 0.5 + (lfo_r - 0.5) * depth;
        let coeffs = [
            self.phase_to_allpass_coeff(sweep_l),
            self.phase_to_allpass_coeff(sweep_r),
        ];

        let mut out = [0.0_f32; 2];
        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let a = coeffs[ch];
            let mut u = flush_denormal(x + fb_gain * self.fb_state[ch]);
            for z in &mut self.ap_z[ch] {
                let y = a * u + *z;
                *z = flush_denormal(u - a * y);
                u = y;
            }
            self.fb_state[ch] = flush_denormal(fast_tanh(u * 0.85) * 1.15);
            let phased = 0.5 * (x + u);
            out[ch] = flush_denormal(x + (phased - x) * mix);
        }

        let activity = (0.5 * (lfo_l + lfo_r) * depth * mix).clamp(0.0, 1.0);
        (out[0], out[1], activity)
    }
}

/// Tempo-synced stereo Delay (`MAIN` = `TIME, FEEDBK, MOD, MIX`, `EQ` = `LOW CUT, HIGH CUT, PING-PONG, WIDTH`).
pub(crate) struct DelayStereo {
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    eq: EqStageStereo,
    write_pos: usize,
    smooth_delay: f32,
    lfo_phase: f32,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl DelayStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        let init_delay = (sr * 0.375).min((MAX_DELAY_SAMPLES - 4) as f32);
        Self {
            buf_l: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            buf_r: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            eq: EqStageStereo::new(sr),
            write_pos: 0,
            smooth_delay: init_delay,
            lfo_phase: 0.0,
            sample_rate: sr,
            dirty: false,
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.eq.set_sample_rate(sr);
    }

    pub(crate) fn reset(&mut self) {
        if self.dirty {
            self.buf_l.fill(0.0);
            self.buf_r.fill(0.0);
            self.dirty = false;
        }
        self.eq.reset();
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub fn sync_index(p0: f32) -> usize {
        ((p0.clamp(0.0, 0.9999) * 8.0) as usize).min(7)
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        tempo_bpm: f32,
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && !self.dirty {
            return (0.0, 0.0, 0.0);
        }

        let idx = Self::sync_index(p[0]);
        let beats = DELAY_SYNC_BEATS[idx];
        let bpm = tempo_bpm.clamp(20.0, 300.0);
        let target_samples = ((60.0 / bpm) * beats * self.sample_rate)
            .clamp(16.0, (MAX_DELAY_SAMPLES - 64) as f32);

        let slew = 1.0 - (-1.0 / (0.025 * self.sample_rate)).exp();
        self.smooth_delay += slew * (target_samples - self.smooth_delay);

        let feedback = (p[1].clamp(0.0, 1.0) * 0.94).min(0.94);
        let mod_amt = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        let low_cut = eq[0].clamp(0.0, 1.0);
        let high_cut = eq[1].clamp(0.0, 1.0);
        let ping_pong = eq[2].clamp(0.0, 1.0);
        let width = eq[3].clamp(0.0, 1.0);

        // Tape/BBD delay time modulation
        self.lfo_phase = (self.lfo_phase + 0.65 / self.sample_rate).fract();
        let mod_samples = mod_amt * (0.0025 * self.sample_rate);
        let d_l = (self.smooth_delay + mod_samples * (self.lfo_phase * std::f32::consts::TAU).sin())
            .clamp(4.0, (MAX_DELAY_SAMPLES - 4) as f32);
        let d_r = (self.smooth_delay * (1.0 - 0.008 * (1.0 - ping_pong))
            + mod_samples * (self.lfo_phase * std::f32::consts::TAU).cos())
        .clamp(4.0, (MAX_DELAY_SAMPLES - 4) as f32);

        let raw_tap_l = read_linear(&self.buf_l, self.write_pos, d_l);
        let raw_tap_r = read_linear(&self.buf_r, self.write_pos, d_r);
        let (tap_l, tap_r) = self.eq.process(raw_tap_l, raw_tap_r, low_cut, high_cut);

        if self.idle_samples > (MAX_DELAY_SAMPLES as u32)
            && raw_tap_l.abs() < 1e-8
            && raw_tap_r.abs() < 1e-8
            && tap_l.abs() < 1e-8
            && tap_r.abs() < 1e-8
        {
            self.reset();
            return (0.0, 0.0, 0.0);
        }

        // Cross-feedback & input injection for Ping-Pong mode
        let in_to_l = in_l * (1.0 - 0.5 * ping_pong) + in_r * (0.5 * ping_pong);
        let in_to_r = in_r * (1.0 - ping_pong);
        let fb_src_l = tap_l * (1.0 - ping_pong) + tap_r * ping_pong;
        let fb_src_r = tap_r * (1.0 - ping_pong) + tap_l * ping_pong;

        let fb_l = fast_tanh((in_to_l + fb_src_l * feedback + tap_r * (feedback * 0.08 * (1.0 - ping_pong))) * 0.5) * 2.0;
        let fb_r = fast_tanh((in_to_r + fb_src_r * feedback + tap_l * (feedback * 0.08 * (1.0 - ping_pong))) * 0.5) * 2.0;

        self.dirty = true;
        self.buf_l[self.write_pos] = fb_l;
        self.buf_r[self.write_pos] = fb_r;
        self.write_pos = (self.write_pos + 1) % MAX_DELAY_SAMPLES;

        let (wide_l, wide_r) = apply_stereo_width(tap_l, tap_r, width);
        let out_l = in_l + wide_l * mix;
        let out_r = in_r + wide_r * mix;
        let activity = (wide_l.abs().max(wide_r.abs()) * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

/// Stereo algorithmic Reverb (`MAIN` = `SIZE, DECAY, DAMP, MIX`, `EQ` = `LOW CUT, HIGH CUT, PRE-DLY, WIDTH`).
pub(crate) struct ReverbStereo {
    pre_buf_l: Box<[f32]>,
    pre_buf_r: Box<[f32]>,
    pre_write: usize,
    comb_l: [Box<[f32]>; 4],
    comb_r: [Box<[f32]>; 4],
    comb_pos_l: [usize; 4],
    comb_pos_r: [usize; 4],
    comb_damp_l: [f32; 4],
    comb_damp_r: [f32; 4],
    ap_l: [Box<[f32]>; 2],
    ap_r: [Box<[f32]>; 2],
    ap_pos_l: [usize; 2],
    ap_pos_r: [usize; 2],
    eq: EqStageStereo,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl ReverbStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            pre_buf_l: vec![0.0; REVERB_PREDELAY_LEN].into_boxed_slice(),
            pre_buf_r: vec![0.0; REVERB_PREDELAY_LEN].into_boxed_slice(),
            pre_write: 0,
            comb_l: std::array::from_fn(|_| vec![0.0; REVERB_COMB_LEN].into_boxed_slice()),
            comb_r: std::array::from_fn(|_| vec![0.0; REVERB_COMB_LEN].into_boxed_slice()),
            comb_pos_l: [0; 4],
            comb_pos_r: [0; 4],
            comb_damp_l: [0.0; 4],
            comb_damp_r: [0.0; 4],
            ap_l: std::array::from_fn(|_| vec![0.0; REVERB_AP_LEN].into_boxed_slice()),
            ap_r: std::array::from_fn(|_| vec![0.0; REVERB_AP_LEN].into_boxed_slice()),
            ap_pos_l: [0; 2],
            ap_pos_r: [0; 2],
            eq: EqStageStereo::new(sr),
            sample_rate: sr,
            dirty: false,
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.eq.set_sample_rate(sr);
    }

    pub(crate) fn reset(&mut self) {
        if self.dirty {
            self.pre_buf_l.fill(0.0);
            self.pre_buf_r.fill(0.0);
            for i in 0..4 {
                self.comb_l[i].fill(0.0);
                self.comb_r[i].fill(0.0);
            }
            for i in 0..2 {
                self.ap_l[i].fill(0.0);
                self.ap_r[i].fill(0.0);
            }
            self.dirty = false;
        }
        self.eq.reset();
        self.pre_write = 0;
        self.comb_pos_l = [0; 4];
        self.comb_pos_r = [0; 4];
        self.comb_damp_l = [0.0; 4];
        self.comb_damp_r = [0.0; 4];
        self.ap_pos_l = [0; 2];
        self.ap_pos_r = [0; 2];
        self.idle_samples = 1024;
    }

    #[inline]
    fn comb_tick(
        buf: &mut [f32],
        pos: &mut usize,
        len: usize,
        input: f32,
        feedback: f32,
        damp: f32,
        filter_store: &mut f32,
    ) -> f32 {
        let cur = *pos % len;
        let out = buf[cur];
        *filter_store = flush_denormal(out * (1.0 - damp) + *filter_store * damp);
        buf[cur] = input + *filter_store * feedback;
        *pos = (cur + 1) % len;
        out
    }

    #[inline]
    fn ap_tick(buf: &mut [f32], pos: &mut usize, len: usize, input: f32) -> f32 {
        let cur = *pos % len;
        let buf_out = buf[cur];
        let y = -input + buf_out;
        buf[cur] = input + buf_out * 0.5;
        *pos = (cur + 1) % len;
        y
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && !self.dirty {
            return (0.0, 0.0, 0.0);
        }

        let size = 0.45 + p[0].clamp(0.0, 1.0) * 0.90;
        let decay = 0.40 + p[1].clamp(0.0, 1.0) * 0.55;
        let damp = 0.08 + p[2].clamp(0.0, 1.0) * 0.72;
        let mix = p[3].clamp(0.0, 1.0);

        let low_cut = eq[0].clamp(0.0, 1.0);
        let high_cut = eq[1].clamp(0.0, 1.0);
        let predelay_s = eq[2].clamp(0.0, 1.0) * 0.250;
        let width = eq[3].clamp(0.0, 1.0);

        self.dirty = true;

        self.pre_buf_l[self.pre_write] = in_l;
        self.pre_buf_r[self.pre_write] = in_r;
        let delay_samples = (predelay_s * self.sample_rate).round() as usize;
        let (feed_l, feed_r) = if delay_samples <= 1 {
            (in_l, in_r)
        } else {
            let d = delay_samples.min(REVERB_PREDELAY_LEN - 2);
            let rd = (self.pre_write + REVERB_PREDELAY_LEN - d) % REVERB_PREDELAY_LEN;
            (self.pre_buf_l[rd], self.pre_buf_r[rd])
        };
        self.pre_write = (self.pre_write + 1) % REVERB_PREDELAY_LEN;

        let mono_in = (feed_l + feed_r) * 0.06;
        let sr_scale = (self.sample_rate / 44_100.0).clamp(0.5, 2.2);

        let mut wet_l = 0.0_f32;
        let mut wet_r = 0.0_f32;
        for i in 0..4 {
            let len_l = ((COMB_BASE[i] as f32 * size * sr_scale) as usize)
                .clamp(64, REVERB_COMB_LEN - 1);
            let len_r = (((COMB_BASE[i] + STEREO_SPREAD) as f32 * size * sr_scale) as usize)
                .clamp(64, REVERB_COMB_LEN - 1);

            wet_l += Self::comb_tick(
                &mut self.comb_l[i],
                &mut self.comb_pos_l[i],
                len_l,
                mono_in,
                decay,
                damp,
                &mut self.comb_damp_l[i],
            );
            wet_r += Self::comb_tick(
                &mut self.comb_r[i],
                &mut self.comb_pos_r[i],
                len_r,
                mono_in,
                decay,
                damp,
                &mut self.comb_damp_r[i],
            );
        }

        for i in 0..2 {
            let ap_len_l = ((AP_BASE[i] as f32 * sr_scale) as usize).clamp(32, REVERB_AP_LEN - 1);
            let ap_len_r = (((AP_BASE[i] + STEREO_SPREAD) as f32 * sr_scale) as usize)
                .clamp(32, REVERB_AP_LEN - 1);
            wet_l = Self::ap_tick(&mut self.ap_l[i], &mut self.ap_pos_l[i], ap_len_l, wet_l);
            wet_r = Self::ap_tick(&mut self.ap_r[i], &mut self.ap_pos_r[i], ap_len_r, wet_r);
        }

        let base_l = (wet_l * 0.85 + wet_r * 0.15) * 1.4;
        let base_r = (wet_r * 0.85 + wet_l * 0.15) * 1.4;
        let (eq_l, eq_r) = self.eq.process(base_l, base_r, low_cut, high_cut);
        let (stereo_l, stereo_r) = apply_stereo_width(eq_l, eq_r, width);

        if self.idle_samples > (REVERB_COMB_LEN as u32 * 4 + REVERB_PREDELAY_LEN as u32)
            && stereo_l.abs() < 1e-8
            && stereo_r.abs() < 1e-8
        {
            self.reset();
            return (0.0, 0.0, 0.0);
        }

        let out_l = in_l + (stereo_l - in_l) * mix;
        let out_r = in_r + (stereo_r - in_r) * mix;
        let activity = (stereo_l.abs().max(stereo_r.abs()) * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

/// High-precision cubic Hermite stereo Flanger (`MAIN` = `RATE, DEPTH, DELAY, MIX`,
/// `MOD` = `FEEDBK, PHASE, LOW CUT, TONE`).
pub(crate) struct FlangerStereo {
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    fb_l: f32,
    fb_r: f32,
    eq: EqStageStereo,
    write_pos: usize,
    lfo_phase: f32,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl FlangerStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            buf_l: vec![0.0; MAX_FLANGE_SAMPLES].into_boxed_slice(),
            buf_r: vec![0.0; MAX_FLANGE_SAMPLES].into_boxed_slice(),
            fb_l: 0.0,
            fb_r: 0.0,
            eq: EqStageStereo::new(sr),
            write_pos: 0,
            lfo_phase: 0.0,
            sample_rate: sr,
            dirty: false,
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.eq.set_sample_rate(sr);
    }

    pub(crate) fn reset(&mut self) {
        if self.dirty {
            self.buf_l.fill(0.0);
            self.buf_r.fill(0.0);
            self.dirty = false;
        }
        self.fb_l = 0.0;
        self.fb_r = 0.0;
        self.eq.reset();
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) {
            if !self.dirty {
                return (0.0, 0.0, 0.0);
            }
            if self.idle_samples > (MAX_FLANGE_SAMPLES as u32 + 64) {
                self.reset();
                return (0.0, 0.0, 0.0);
            }
        }

        let rate_hz = flanger_rate_hz(p[0]);
        let depth = p[1].clamp(0.0, 1.0);
        let center_ms = flanger_delay_ms(p[2]);
        let mix = p[3].clamp(0.0, 1.0);

        let fb = flanger_feedback(eq[0]);
        let phase_rad = eq[1].clamp(0.0, 1.0) * std::f32::consts::PI;
        let low_cut = eq[2].clamp(0.0, 1.0);
        let tone = eq[3].clamp(0.0, 1.0);

        self.dirty = true;

        self.lfo_phase = (self.lfo_phase + rate_hz / self.sample_rate).fract();
        let ang_l = self.lfo_phase * std::f32::consts::TAU;
        let ang_r = ang_l + phase_rad;

        let sr_ms = self.sample_rate * 0.001;
        let max_d = (MAX_FLANGE_SAMPLES - 4) as f32;
        let d_ms_l = (center_ms * 4.0_f32.powf(depth * 0.65 * ang_l.sin())).clamp(0.08, 19.5);
        let d_ms_r = (center_ms * 4.0_f32.powf(depth * 0.65 * ang_r.sin())).clamp(0.08, 19.5);
        let d_samp_l = (d_ms_l * sr_ms).clamp(1.0, max_d);
        let d_samp_r = (d_ms_r * sr_ms).clamp(1.0, max_d);

        // Soft-limit feedback injection so +-99% feedback sweeps stay bounded and musical
        let in_fb_l = fast_tanh((in_l + self.fb_l * fb) * 0.72) * 1.38;
        let in_fb_r = fast_tanh((in_r + self.fb_r * fb) * 0.72) * 1.38;

        let tap_l = read_hermite(&self.buf_l, self.write_pos, d_samp_l);
        let tap_r = read_hermite(&self.buf_r, self.write_pos, d_samp_r);
        let (flt_l, flt_r) = self.eq.process(tap_l, tap_r, low_cut, tone);

        self.buf_l[self.write_pos] = flush_denormal(in_fb_l);
        self.buf_r[self.write_pos] = flush_denormal(in_fb_r);
        self.fb_l = flush_denormal(flt_l);
        self.fb_r = flush_denormal(flt_r);
        self.write_pos = (self.write_pos + 1) % MAX_FLANGE_SAMPLES;

        let flanged_l = 0.5 * (in_l + flt_l);
        let flanged_r = 0.5 * (in_r + flt_r);
        let out_l = in_l + (flanged_l - in_l) * mix;
        let out_r = in_r + (flanged_r - in_r) * mix;
        let activity =
            (0.5 * (ang_l.sin().abs() + ang_r.sin().abs()) * mix * (0.35 + 0.65 * fb.abs()))
                .clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}
