// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The multimode filter: a stereo pair of zero-delay-feedback state-variable filters, one stage (12 dB per octave) or two
//! (24), low, band or high pass, with an overdrive in front; and a Moog-style transistor ladder (types 3 to 5), four
//! one-pole stages in a feedback loop with a saturation in front of every stage.
//!
//! ## The ladder
//!
//! Each stage is a trapezoidal (TPT) one-pole, `y = G x' + s / (1 + g)` with `g = tan(pi fc / fs)`, `G = g / (1 + g)`
//! and `x'` the stage's saturated input. The loop `u = x - k y4` (`k = 4.1 res`, so `res = 1` self-oscillates at the cutoff)
//! has no unit delay: it is solved in the same sample. The saturation `tanh` in front of each stage is replaced by a gain
//! `t = tanh(v) / v` of the stage's predicted input `v` (a first pass with `t = 1` gives the prediction), and the loop is
//! then solved again, exactly, for those gains: `u = (x - k S) / (1 + k P)` where `P = G^4 t1 t2 t3 t4` and `S` collects the
//! stored states. This is one explicit pass of a Newton iteration on the zero-delay loop; it keeps the cutoff and the
//! resonance where they are set (no half-sample shift of the resonant peak) and stays bounded because every `t <= 1`.
//! `tanh` is the rational form of [`crate::util::soft`], whose gain `t` needs no division by `v`.
//!
//! ## Limits, measured
//!
//! - The saturation is not oversampled. A 2x oversampled run (31 tap half-band up and down, measured in the tests) lowers
//!   the aliasing of a driven sine by about 27 to 33 dB (drive 1: -27 dB at 1x against -55 dB at 2x with the cutoff at
//!   12 kHz; drive 0.5: -49 dB against -81 dB). It costs a ladder tick at twice the rate plus two 31 tap filters per
//!   sample, many times the filter's own cost on 16 voices and two channels, so it is not switched on; the in-loop
//!   low pass already removes what lies above the cutoff, and only a bright, hard-driven patch shows the aliasing.
//! - The `res` knob maps to `k = 4.1 res`: self-oscillation starts at about 0.976 and the amplitude is held to about 0.2
//!   by the stage saturation. With no input a voice's tail still ends 60 ms after its amplitude envelope does.
//! - The one-pass saturation prediction is not a converged Newton solve; with every gain `<= 1` it cannot overshoot,
//!   which the audio-rate cutoff modulation test checks at full resonance and drive.

use std::f32::consts::PI;

use crate::util::soft;

/// The highest filter type: 0 low, 1 band, 2 high (state variable); 3 ladder low, 4 ladder band, 5 ladder high.
pub const MAX_TYPE: u8 = 5;
/// The first ladder type.
pub const LADDER: u8 = 3;

#[derive(Clone, Copy, Debug, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

/// One stage's coefficients.
#[derive(Clone, Copy, Debug, Default)]
pub struct Coef {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Coef {
    pub fn new(hz: f32, k: f32, sample_rate: f32) -> Coef {
        let g = (PI * (hz / sample_rate).clamp(1e-4, 0.49)).tan();
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Coef {
            k,
            a1,
            a2,
            a3: g * a2,
        }
    }
}

impl Svf {
    /// (low, band, high) for input `x`.
    #[inline]
    fn tick(&mut self, x: f32, c: &Coef) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - c.k * v1 - v2)
    }
}

/// The ladder's coefficients for one control step.
#[derive(Clone, Copy, Debug, Default)]
pub struct LadderCoef {
    /// `G = g / (1 + g)`, the instantaneous gain of one stage.
    big_g: f32,
    /// `1 / (1 + g)`, what a stage keeps of its state.
    keep: f32,
    /// The loop gain: 4 is the edge of self-oscillation.
    k: f32,
    /// The make-up gain that gives back part of the passband the feedback takes.
    makeup: f32,
}

impl LadderCoef {
    pub fn new(hz: f32, res: f32, sample_rate: f32) -> LadderCoef {
        let g = (PI * (hz / sample_rate).clamp(1e-4, 0.49)).tan();
        let k = 4.1 * res.clamp(0.0, 1.0);
        LadderCoef {
            big_g: g / (1.0 + g),
            keep: 1.0 / (1.0 + g),
            k,
            makeup: 1.0 + 0.5 * k,
        }
    }
}

/// `tanh(v) / v`, the gain a saturated stage has at the input level `v` (rational, no division by `v`).
#[inline]
fn sat_gain(v: f32) -> f32 {
    let a = v * v;
    if a < 9.0 {
        (27.0 + a) / (27.0 + 9.0 * a)
    } else {
        1.0 / a.sqrt()
    }
}

/// Flushes denormals; the states of a ringing filter decay to nothing and must not slow the CPU down.
#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < 1e-18 { 0.0 } else { x }
}

/// One channel's ladder: the four stage states.
#[derive(Clone, Copy, Debug, Default)]
struct Ladder {
    s: [f32; 4],
}

impl Ladder {
    /// The four stage outputs and the loop node `u` for input `x`: `([y1, y2, y3, y4], u)`.
    #[inline]
    fn tick(&mut self, x: f32, c: &LadderCoef) -> ([f32; 4], f32) {
        let g = c.big_g;
        let b = [
            self.s[0] * c.keep,
            self.s[1] * c.keep,
            self.s[2] * c.keep,
            self.s[3] * c.keep,
        ];
        // Pass 1: the linear loop, to predict what every stage will see.
        let g2 = g * g;
        let sum = g2 * g * b[0] + g2 * b[1] + g * b[2] + b[3];
        let u0 = (x - c.k * sum) / (1.0 + c.k * g2 * g2);
        let y1p = g * u0 + b[0];
        let y2p = g * y1p + b[1];
        let y3p = g * y2p + b[2];
        // Pass 2: the loop again with each stage's saturation gain.
        let a1 = g * sat_gain(u0);
        let a2 = g * sat_gain(y1p);
        let a3 = g * sat_gain(y2p);
        let a4 = g * sat_gain(y3p);
        let a34 = a3 * a4;
        let sum = a2 * a34 * b[0] + a34 * b[1] + a4 * b[2] + b[3];
        let u = (x - c.k * sum) / (1.0 + c.k * a1 * a2 * a34);
        let y1 = a1 * u + b[0];
        let y2 = a2 * y1 + b[1];
        let y3 = a3 * y2 + b[2];
        let y4 = a4 * y3 + b[3];
        // The trapezoidal state update `s = 2 y - s`, with the denormals flushed.
        self.s[0] = flush(2.0 * y1 - self.s[0]);
        self.s[1] = flush(2.0 * y2 - self.s[1]);
        self.s[2] = flush(2.0 * y3 - self.s[2]);
        self.s[3] = flush(2.0 * y4 - self.s[3]);
        ([y1, y2, y3, y4], u)
    }
}

/// The damping `k` (1 / Q) for a resonance in `[0, 1]`.
pub fn damping(res: f32) -> f32 {
    1.4 * (1.0 - res.clamp(0.0, 1.0)).powf(1.3) + 0.08
}

/// What a filter does with its input.
#[derive(Clone, Copy, Debug)]
pub struct Setup {
    /// 0 low, 1 band, 2 high.
    pub mode: u8,
    pub slope24: bool,
    pub first: Coef,
    pub second: Coef,
    pub ladder: LadderCoef,
    pub drive: f32,
}

impl Setup {
    pub fn new(mode: u8, slope24: bool, hz: f32, res: f32, drive: f32, sample_rate: f32) -> Setup {
        if mode >= LADDER {
            return Setup {
                mode,
                slope24,
                first: Coef::default(),
                second: Coef::default(),
                ladder: LadderCoef::new(hz, res, sample_rate),
                drive,
            };
        }
        let k = damping(res);
        Setup {
            mode,
            slope24,
            first: Coef::new(hz, k, sample_rate),
            // The second stage stays plain, so 24 dB does not square the resonance peak.
            second: Coef::new(hz, 1.0, sample_rate),
            ladder: LadderCoef::default(),
            drive,
        }
    }
}

/// One voice's filter state, both channels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Filter {
    s: [[Svf; 2]; 2],
    lad: [Ladder; 2],
}

impl Filter {
    pub fn reset(&mut self) {
        *self = Filter::default();
    }

    /// One sample of channel `ch` (0 or 1).
    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32, f: &Setup) -> f32 {
        if f.mode >= LADDER {
            return self.ladder(ch, x, f);
        }
        let x = if f.drive > 0.0 {
            soft(x * (1.0 + f.drive * 6.0)) * (1.0 - 0.3 * f.drive)
        } else {
            x
        };
        let pick = |(lp, bp, hp): (f32, f32, f32), k: f32| match f.mode {
            0 => lp,
            1 => bp * k,
            _ => hp,
        };
        let y = pick(self.s[ch][0].tick(x, &f.first), f.first.k);
        if f.slope24 {
            pick(self.s[ch][1].tick(y, &f.second), f.second.k)
        } else {
            y
        }
    }

    /// The ladder: the drive pushes the input into the stages' saturation, so it bends the sound where a transistor
    /// ladder does; the output gives back part of the level.
    #[inline]
    fn ladder(&mut self, ch: usize, x: f32, f: &Setup) -> f32 {
        let c = &f.ladder;
        let ([y1, y2, y3, y4], u) = self.lad[ch].tick(x * (1.0 + 5.0 * f.drive), c);
        let y = match f.mode {
            LADDER => {
                if f.slope24 {
                    y4
                } else {
                    y2
                }
            }
            // Four poles of band pass: peak gain 1 at the cutoff without feedback.
            4 => 4.0 * (y2 - 2.0 * y3 + y4),
            _ => u - 4.0 * y1 + 6.0 * y2 - 4.0 * y3 + y4,
        };
        y * c.makeup / (1.0 + 1.5 * f.drive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain(mode: u8, slope24: bool, cutoff: f32, test_hz: f32) -> f32 {
        let sr = 48_000.0;
        let setup = Setup::new(mode, slope24, cutoff, 0.0, 0.0, sr);
        let mut f = Filter::default();
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = (2.0 * PI * test_hz * i as f32 / sr).sin();
            let y = f.tick(0, x, &setup);
            if i > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn low_pass_passes_lows_and_cuts_highs() {
        assert!(gain(0, false, 1000.0, 100.0) > 0.95);
        let g12 = gain(0, false, 1000.0, 8000.0);
        let g24 = gain(0, true, 1000.0, 8000.0);
        assert!(g12 < 0.05, "{g12}");
        assert!(g24 < g12 * 0.5, "24 dB cuts more: {g24} vs {g12}");
    }

    #[test]
    fn high_pass_cuts_lows() {
        assert!(gain(2, false, 4000.0, 100.0) < 0.05);
        assert!(gain(2, false, 1000.0, 10_000.0) > 0.9);
    }

    #[test]
    fn band_pass_peaks_at_the_cutoff() {
        let at = gain(1, false, 2000.0, 2000.0);
        assert!(at > gain(1, false, 2000.0, 200.0) * 2.0);
        assert!(at > gain(1, false, 2000.0, 15_000.0) * 2.0);
    }

    #[test]
    fn resonance_stays_stable() {
        let sr = 48_000.0;
        let setup = Setup::new(0, true, 5000.0, 1.0, 1.0, sr);
        let mut f = Filter::default();
        let mut peak = 0.0f32;
        for i in 0..96_000 {
            let x = if i % 480 == 0 { 1.0 } else { 0.0 };
            peak = peak.max(f.tick(0, x, &setup).abs());
        }
        assert!(peak.is_finite() && peak < 20.0, "{peak}");
    }

    // ---- The ladder: measured, not assumed. ----

    const SR: f32 = 48_000.0;

    /// Steady-state gain of a ladder type for a sine of `amp` at `hz`.
    fn ladder_gain(
        mode: u8,
        slope24: bool,
        cutoff: f32,
        res: f32,
        drive: f32,
        hz: f32,
        amp: f32,
    ) -> f32 {
        let setup = Setup::new(mode, slope24, cutoff, res, drive, SR);
        let mut f = Filter::default();
        let mut peak = 0.0f32;
        let n = 48_000;
        for i in 0..n {
            let x = amp
                * (2.0 * std::f64::consts::PI * f64::from(hz) * i as f64 / f64::from(SR)).sin()
                    as f32;
            let y = f.tick(0, x, &setup);
            assert!(y.is_finite());
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak / amp
    }

    /// The exact response of the discrete loop `H^4 / (1 + k H^4)` (`H` the trapezoidal one-pole), times the make-up.
    fn ladder_theory(cutoff: f32, res: f32, hz: f32) -> f32 {
        let c = LadderCoef::new(cutoff, res, SR);
        let g = (PI * cutoff / SR).tan();
        let w = 2.0 * PI * hz / SR;
        let (zr, zi) = (w.cos(), -w.sin()); // z^-1
        let mul = |a: (f32, f32), b: (f32, f32)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
        let num = (c.big_g * (1.0 + zr), c.big_g * zi);
        let den = (
            1.0 - (1.0 - g) / (1.0 + g) * zr,
            -(1.0 - g) / (1.0 + g) * zi,
        );
        let d2 = den.0 * den.0 + den.1 * den.1;
        let h = (
            (num.0 * den.0 + num.1 * den.1) / d2,
            (num.1 * den.0 - num.0 * den.1) / d2,
        );
        let h2 = mul(h, h);
        let h4 = mul(h2, h2);
        let d = (1.0 + c.k * h4.0, c.k * h4.1);
        let m = (h4.0 * h4.0 + h4.1 * h4.1).sqrt() / (d.0 * d.0 + d.1 * d.1).sqrt();
        m * c.makeup
    }

    #[test]
    fn ladder_response_matches_the_exact_discrete_loop() {
        // Small signals stay in the linear part of the saturation, so the measured gain must equal the theory.
        for (cutoff, res) in [(1000.0, 0.0), (1000.0, 0.5), (3000.0, 0.8), (500.0, 0.9)] {
            for mult in [0.1f32, 0.5, 0.9, 1.0, 1.1, 2.0, 4.0] {
                let hz = cutoff * mult;
                let got = ladder_gain(LADDER, true, cutoff, res, 0.0, hz, 0.002);
                let want = ladder_theory(cutoff, res, hz);
                assert!(
                    (got - want).abs() < 0.04 * want.max(0.05),
                    "fc {cutoff} res {res} at {hz}: {got} vs {want}"
                );
            }
        }
    }

    #[test]
    fn ladder_low_pass_is_four_poles() {
        let at_cutoff = ladder_gain(LADDER, true, 1000.0, 0.0, 0.0, 1000.0, 0.002);
        assert!((at_cutoff - 0.25).abs() < 0.02, "{at_cutoff}"); // four -3 dB poles
        // Well below Nyquist, where the bilinear warp is small: 24 dB per octave.
        let a = ladder_gain(LADDER, true, 100.0, 0.0, 0.0, 800.0, 0.002);
        let b = ladder_gain(LADDER, true, 100.0, 0.0, 0.0, 1600.0, 0.002);
        let octave_db = 20.0 * (a / b).log10();
        assert!(
            (23.0..25.0).contains(&octave_db),
            "{octave_db} dB per octave"
        );
        assert!(ladder_gain(LADDER, true, 5000.0, 0.0, 0.0, 100.0, 0.002) > 0.98);
        // The 12 dB tap falls half as fast.
        let a = ladder_gain(LADDER, false, 100.0, 0.0, 0.0, 800.0, 0.002);
        let b = ladder_gain(LADDER, false, 100.0, 0.0, 0.0, 1600.0, 0.002);
        let octave_db = 20.0 * (a / b).log10();
        assert!(
            (11.0..13.0).contains(&octave_db),
            "{octave_db} dB per octave"
        );
    }

    #[test]
    fn ladder_resonance_peaks_at_the_cutoff_and_nearly_oscillates() {
        let mut best = (0.0f32, 0.0f32);
        for i in 0..200 {
            let hz = 300.0 + i as f32 * 7.0;
            let g = ladder_gain(LADDER, true, 1000.0, 0.9, 0.0, hz, 0.002);
            if g > best.0 {
                best = (g, hz);
            }
        }
        assert!(best.0 > 5.0, "peak {}", best.0);
        assert!((best.1 - 1000.0).abs() < 60.0, "peak at {}", best.1);
        // More resonance, more peak.
        assert!(
            ladder_gain(LADDER, true, 1000.0, 0.95, 0.0, 1000.0, 0.002)
                > ladder_gain(LADDER, true, 1000.0, 0.5, 0.0, 1000.0, 0.002) * 3.0
        );
    }

    #[test]
    fn ladder_self_oscillates_at_the_cutoff_and_stays_bounded() {
        for cutoff in [200.0f32, 1000.0, 4000.0] {
            let setup = Setup::new(LADDER, true, cutoff, 1.0, 0.0, SR);
            let mut f = Filter::default();
            let n = 96_000;
            let (mut crossings, mut prev, mut peak) = (0u32, 0.0f32, 0.0f32);
            for i in 0..n {
                let x = if i == 0 { 0.1 } else { 0.0 };
                let y = f.tick(0, x, &setup);
                assert!(y.is_finite());
                if i > n / 2 {
                    peak = peak.max(y.abs());
                    if prev <= 0.0 && y > 0.0 {
                        crossings += 1;
                    }
                }
                prev = y;
            }
            assert!(peak > 0.05 && peak < 4.0, "fc {cutoff}: amplitude {peak}");
            let hz = crossings as f32 / ((n / 2) as f32 / SR);
            assert!(
                (hz / cutoff - 1.0).abs() < 0.04,
                "fc {cutoff}: oscillates at {hz}"
            );
        }
    }

    #[test]
    fn ladder_saturates_inside_the_loop() {
        // The same filter is linear for small signals and compresses loud ones.
        let small = ladder_gain(LADDER, true, 1000.0, 0.3, 0.0, 300.0, 0.002);
        let loud = ladder_gain(LADDER, true, 1000.0, 0.3, 0.0, 300.0, 6.0);
        assert!(loud < small * 0.5, "{loud} vs {small}");
        // Driven hard, the output stays bounded.
        let hard = ladder_gain(LADDER, true, 1000.0, 0.7, 1.0, 300.0, 8.0) * 8.0;
        assert!(hard.is_finite() && hard < 12.0, "{hard}");
        // A saturated loop adds harmonics: a sine in gives a third harmonic out.
        let setup = Setup::new(LADDER, true, 8000.0, 0.0, 1.0, SR);
        let mut f = Filter::default();
        let (mut s1, mut c1, mut s3, mut c3) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let hz = 400.0f32;
        for i in 0..48_000 {
            let ph = 2.0 * std::f64::consts::PI * f64::from(hz) * i as f64 / f64::from(SR);
            let y = f.tick(0, ph.sin() as f32, &setup);
            if i >= 24_000 {
                s1 += y * ph.sin() as f32;
                c1 += y * ph.cos() as f32;
                s3 += y * (3.0 * ph).sin() as f32;
                c3 += y * (3.0 * ph).cos() as f32;
            }
        }
        let h1 = (s1 * s1 + c1 * c1).sqrt();
        let h3 = (s3 * s3 + c3 * c3).sqrt();
        assert!(h3 / h1 > 0.02, "third harmonic {}", h3 / h1);
    }

    #[test]
    fn ladder_stays_stable_under_audio_rate_cutoff_modulation() {
        let mut rng = crate::util::Rng::new(7);
        for res in [0.0f32, 0.7, 1.0] {
            for drive in [0.0f32, 1.0] {
                let mut f = Filter::default();
                let mut peak = 0.0f32;
                for i in 0..96_000 {
                    // A new cutoff every sample, anywhere from 20 Hz to nearly Nyquist, on noise at full scale.
                    let hz = 20.0 * 1000.0f32.powf(rng.unit());
                    let setup = Setup::new(LADDER, true, hz.min(0.45 * SR), res, drive, SR);
                    let y = f.tick(0, rng.bipolar(), &setup);
                    assert!(y.is_finite(), "res {res} drive {drive} at {i}");
                    peak = peak.max(y.abs());
                }
                assert!(peak < 30.0, "res {res} drive {drive}: peak {peak}");
            }
        }
    }

    #[test]
    fn ladder_band_and_high_pass_have_their_shapes() {
        assert!(ladder_gain(5, true, 4000.0, 0.0, 0.0, 100.0, 0.002) < 0.01);
        assert!(ladder_gain(5, true, 1000.0, 0.0, 0.0, 15_000.0, 0.002) > 0.9);
        let at = ladder_gain(4, true, 2000.0, 0.0, 0.0, 2000.0, 0.002);
        assert!((at - 1.0).abs() < 0.08, "band peak {at}");
        assert!(ladder_gain(4, true, 2000.0, 0.0, 0.0, 100.0, 0.002) < 0.05);
        assert!(ladder_gain(4, true, 2000.0, 0.0, 0.0, 15_000.0, 0.002) < 0.1);
    }

    #[test]
    fn ladder_states_never_go_denormal_and_silence_is_silent() {
        let setup = Setup::new(LADDER, true, 300.0, 0.4, 0.0, SR);
        let mut f = Filter::default();
        f.tick(0, 1.0, &setup);
        for _ in 0..480_000 {
            f.tick(0, 0.0, &setup);
        }
        for v in f.lad[0].s {
            assert!(v == 0.0 || v.abs() >= 1e-18, "{v}");
        }
        assert_eq!(f.tick(0, 0.0, &setup), 0.0);
        // The channels are independent.
        assert_eq!(f.tick(1, 0.0, &setup), 0.0);
    }

    #[test]
    fn ladder_costs_about_as_much_as_two_state_variable_stages() {
        // A rough guard, not a benchmark: one second of audio for 16 voices x 2 channels is far from real time limits.
        let setup = Setup::new(LADDER, true, 1500.0, 0.6, 0.3, SR);
        let svf = Setup::new(0, true, 1500.0, 0.6, 0.3, SR);
        let mut f = [Filter::default(); 32];
        let t = std::time::Instant::now();
        let mut acc = 0.0f32;
        for i in 0..48_000 {
            for (v, f) in f.iter_mut().enumerate() {
                acc += f.tick(v & 1, ((i + v) as f32 * 0.01).sin(), &setup);
            }
        }
        let ladder = t.elapsed();
        let t = std::time::Instant::now();
        for i in 0..48_000 {
            for (v, f) in f.iter_mut().enumerate() {
                acc += f.tick(v & 1, ((i + v) as f32 * 0.01).sin(), &svf);
            }
        }
        let plain = t.elapsed();
        assert!(acc.is_finite());
        eprintln!("ladder {ladder:?} vs svf {plain:?} for 32 filters x 1 s");
        assert!(ladder < plain * 6 + std::time::Duration::from_millis(50));
    }

    // ---- Oversampling, investigated (not shipped; see the module notes). ----

    /// A 2x oversampled ladder for the measurement only: a 31 tap windowed-sinc half-band up and down around the same
    /// filter run at twice the rate.
    fn oversampled_run(x: &[f32], cutoff: f32, res: f32, drive: f32) -> Vec<f32> {
        const TAPS: usize = 31;
        let h: Vec<f32> = (0..TAPS)
            .map(|i| {
                let m = i as f64 - (TAPS as f64 - 1.0) / 2.0;
                let sinc = if m == 0.0 {
                    0.5
                } else {
                    (std::f64::consts::PI * m / 2.0).sin() / (std::f64::consts::PI * m)
                };
                let win = 0.5
                    + 0.5 * (std::f64::consts::PI * m / ((TAPS as f64 - 1.0) / 2.0 + 1.0)).cos();
                (sinc * win) as f32
            })
            .collect();
        let setup = Setup::new(LADDER, true, cutoff, res, drive, 2.0 * SR);
        let mut f = Filter::default();
        let mut up = [0.0f32; TAPS];
        let mut down = [0.0f32; TAPS];
        let mut out = Vec::with_capacity(x.len());
        for &v in x {
            let mut pair = [0.0f32; 2];
            for (k, slot) in pair.iter_mut().enumerate() {
                // Zero-stuffed input (gain 2 to keep the level), interpolated by the half-band.
                up.rotate_right(1);
                up[0] = if k == 0 { 2.0 * v } else { 0.0 };
                let interp: f32 = up.iter().zip(&h).map(|(a, b)| a * b).sum();
                let y = f.tick(0, interp, &setup);
                down.rotate_right(1);
                down[0] = y;
                *slot = down.iter().zip(&h).map(|(a, b)| a * b).sum();
            }
            out.push(pair[0]);
        }
        out
    }

    #[test]
    fn oversampling_the_ladder_is_measured_before_it_is_chosen() {
        let n = 16_384;
        let bin = 1056.0f64; // 3093.75 Hz: a harmonic lands in a bin centre
        let hz = (bin * f64::from(SR) / n as f64) as f32;
        let inharmonic = |y: &[f32]| {
            let spec = {
                let n = y.len();
                let mut re: Vec<f64> = (0..n)
                    .map(|i| {
                        f64::from(y[i])
                            * (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos())
                    })
                    .collect();
                // Direct DFT of just the bins asked for is enough: harmonic bins and a grid of inharmonic ones.
                let mut mags = vec![0.0f64; n / 2];
                for (k, m) in mags.iter_mut().enumerate().skip(2).step_by(3) {
                    let (mut a, mut b) = (0.0, 0.0);
                    for (i, v) in re.iter().enumerate() {
                        let ph = -2.0 * std::f64::consts::PI * (k * i % n) as f64 / n as f64;
                        a += v * ph.cos();
                        b += v * ph.sin();
                    }
                    *m = (a * a + b * b).sqrt();
                }
                re.clear();
                mags
            };
            let (mut bad, mut all) = (0.0, 0.0);
            for (k, m) in spec.iter().enumerate().skip(2).step_by(3) {
                let h = (k as f64 / bin).round();
                let off = (k as f64 - h * bin).abs();
                all += m * m;
                if h < 1.0 || off > 3.0 {
                    bad += m * m;
                }
            }
            bad / all
        };
        let input: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * f64::from(hz) * i as f64 / f64::from(SR)).sin() as f32
            })
            .collect();
        for (cutoff, drive) in [(12_000.0f32, 1.0f32), (6_000.0, 1.0), (12_000.0, 0.5)] {
            let setup = Setup::new(LADDER, true, cutoff, 0.3, drive, SR);
            let mut f = Filter::default();
            let plain: Vec<f32> = input.iter().map(|x| f.tick(0, *x, &setup)).collect();
            let over = oversampled_run(&input, cutoff, 0.3, drive);
            let (a, b) = (inharmonic(&plain), inharmonic(&over));
            eprintln!(
                "ladder fc {cutoff} drive {drive}: aliasing 1x {:.1} dB, 2x {:.1} dB",
                10.0 * a.log10(),
                10.0 * b.log10()
            );
            assert!(a.is_finite() && b.is_finite());
            assert!(
                b <= a * 1.05,
                "oversampling must never make it worse: {a} vs {b}"
            );
        }
    }
}
