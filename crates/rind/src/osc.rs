// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Band-limited oscillators (PolyBLEP for saw and pulse, PolyBLAMP for the triangle) with up to eight unison voices, each
//! spread in pitch and across the stereo field.
//!
//! Waves 0 to 7 are the original set and sound as they always did. The appended ones: 8 `PWM` (a pulse whose width the
//! modulation matrix moves), 9 `Saw+Sub` and 10 `Pulse+Sub` (the wave with a sub oscillator an octave down, a sine under
//! the saw and a square under the pulse, one per oscillator whatever the unison count), and 11 `SuperSaw`, a saw whose
//! unison follows the law of the classic supersaw: detune offsets that grow faster than linearly towards the outer voices
//! and a centre voice louder than the sides, the sum normalised to the same power as any other.

use crate::util::{Rng, fast_sin, pan_gains};

pub const MAX_UNISON: usize = 8;

/// The first of the appended waves.
pub const WAVE_PWM: u8 = 8;
pub const WAVE_SAW_SUB: u8 = 9;
pub const WAVE_PULSE_SUB: u8 = 10;
pub const WAVE_SUPERSAW: u8 = 11;

/// The level of the sub oscillator against the oscillator's own sum (after the sum's normalisation).
const SUB_LEVEL: f32 = 0.6;

/// How wide the pulse may get: the BLEP of both edges must fit apart.
fn safe_width(w: f32, dt: f32) -> f32 {
    if dt >= 0.25 {
        0.5
    } else {
        w.clamp(2.0 * dt, 1.0 - 2.0 * dt)
    }
}

/// The residual of a unit step at phase 0, for a sample at phase `t` and phase increment `dt`.
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// The residual of a unit slope change at phase 0 (in units of `dt`).
fn blamp(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt - 1.0;
        -(1.0 / 3.0) * x * x * x
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt + 1.0;
        (1.0 / 3.0) * x * x * x
    } else {
        0.0
    }
}

fn wrap(x: f32) -> f32 {
    if x >= 1.0 {
        x - 1.0
    } else if x < 0.0 {
        x + 1.0
    } else {
        x
    }
}

fn saw(p: f32, dt: f32) -> f32 {
    2.0 * p - 1.0 - blep(p, dt)
}

fn tri(p: f32, dt: f32) -> f32 {
    let naive = 4.0 * (p - 0.5).abs() - 1.0;
    naive - 8.0 * dt * blamp(p, dt) + 8.0 * dt * blamp(wrap(p - 0.5), dt)
}

fn pulse(p: f32, dt: f32, w: f32) -> f32 {
    let naive = if p < w { 1.0 } else { -1.0 };
    let y = naive + blep(p, dt) - blep(wrap(p - w), dt);
    (y - (2.0 * w - 1.0)) / (2.0 * w.max(1.0 - w))
}

/// One sample of waveform `wave` (see `WAVE_NAMES`) at phase `p` in `[0, 1)`.
#[inline]
fn sample(wave: u8, p: f32, dt: f32, width: f32, rng: &mut Rng) -> f32 {
    match wave {
        WAVE_PWM | WAVE_PULSE_SUB => pulse(p, dt, safe_width(width, dt)),
        WAVE_SAW_SUB | WAVE_SUPERSAW => saw(p, dt),
        0 => fast_sin(p),
        1 => saw(p, dt),
        2 => tri(p, dt),
        3 => pulse(p, dt, 0.5),
        4 => pulse(p, dt, 0.25),
        5 => pulse(p, dt, 0.125),
        6 => 0.5 * (saw(p, dt) + tri(p, dt)),
        _ => rng.bipolar() * 0.7,
    }
}

/// One oscillator's unison voices.
#[derive(Clone, Debug)]
pub struct Osc {
    phase: [f32; MAX_UNISON],
    dt: [f32; MAX_UNISON],
    ratio: [f32; MAX_UNISON],
    /// Each voice's level in the sum (the supersaw law weighs the centre), before the pan.
    weight: [f32; MAX_UNISON],
    pgl: [f32; MAX_UNISON],
    pgr: [f32; MAX_UNISON],
    n: usize,
    /// What `ratio`, `weight` and the pan gains were computed for, to compute them again only when it moves.
    ratio_key: (usize, f32, bool),
    pan_key: (usize, f32, f32, bool),
    /// The sub oscillator: its phase, increment and level (in the sum's units).
    sub_phase: f32,
    sub_dt: f32,
    sub_gain: f32,
}

impl Default for Osc {
    fn default() -> Self {
        Osc {
            phase: [0.0; MAX_UNISON],
            dt: [0.0; MAX_UNISON],
            ratio: [1.0; MAX_UNISON],
            weight: [1.0; MAX_UNISON],
            pgl: [1.0; MAX_UNISON],
            pgr: [1.0; MAX_UNISON],
            n: 0,
            ratio_key: (0, f32::NAN, false),
            pan_key: (0, f32::NAN, f32::NAN, false),
            sub_phase: 0.0,
            sub_dt: 0.0,
            sub_gain: 0.0,
        }
    }
}

/// A unison voice's place in `[-1, 1]`.
fn spread(i: usize, n: usize) -> f32 {
    if n <= 1 {
        0.0
    } else {
        -1.0 + 2.0 * i as f32 / (n - 1) as f32
    }
}

impl Osc {
    /// Starts the voices' phases: `rand` of a random turn each.
    pub fn start(&mut self, rand: f32, rng: &mut Rng) {
        for p in &mut self.phase {
            *p = rand * rng.unit();
        }
        self.sub_phase = 0.0;
    }

    /// Sets `n` voices (`0..=8`) of base phase increment `base_dt`, `detune_semis` apart at the outer voices, spread over
    /// `stereo` around `pan`, for wave `wave` (which picks the unison law and the sub oscillator). Returns the normalising
    /// gain for the sum.
    pub fn configure(
        &mut self,
        wave: u8,
        n: usize,
        base_dt: f32,
        detune_semis: f32,
        stereo: f32,
        pan: f32,
    ) -> f32 {
        let n = n.min(MAX_UNISON);
        self.n = n;
        let law = wave == WAVE_SUPERSAW;
        if self.ratio_key != (n, detune_semis, law) {
            self.ratio_key = (n, detune_semis, law);
            for i in 0..n {
                let x = spread(i, n);
                // The supersaw's outer voices sit disproportionately far out (offsets of a 7 voice supersaw go like
                // 0, 0.18, 0.57, 1 of the widest).
                let x = if law {
                    x.signum() * x.abs().powf(1.6)
                } else {
                    x
                };
                self.ratio[i] = (detune_semis * x * (1.0 / 12.0)).exp2();
                self.weight[i] = if law {
                    1.0 - 0.45 * spread(i, n).abs()
                } else {
                    1.0
                };
            }
        }
        if self.pan_key != (n, stereo, pan, law) {
            self.pan_key = (n, stereo, pan, law);
            for i in 0..n {
                let (l, r) = pan_gains(pan + spread(i, n) * stereo);
                self.pgl[i] = l * self.weight[i];
                self.pgr[i] = r * self.weight[i];
            }
        }
        for i in 0..n {
            self.dt[i] = (base_dt * self.ratio[i]).min(0.45);
        }
        self.sub_dt = (0.5 * base_dt).min(0.45);
        let norm = if n == 0 {
            0.0
        } else {
            // Equal power whatever the law: the weights are folded into the normalisation.
            1.0 / self.weight[..n].iter().map(|w| w * w).sum::<f32>().sqrt()
        };
        self.sub_gain = if matches!(wave, WAVE_SAW_SUB | WAVE_PULSE_SUB) && norm > 0.0 {
            SUB_LEVEL / norm
        } else {
            0.0
        };
        norm
    }

    pub fn voices(&self) -> usize {
        self.n
    }

    /// One stereo sample, the voices summed. `phase_off` is in `[0, 1)`, `width` the pulse width of the pulse waves.
    #[inline]
    pub fn tick(&mut self, wave: u8, phase_off: f32, width: f32, rng: &mut Rng) -> (f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        for i in 0..self.n {
            let p = self.phase[i];
            let dt = self.dt[i];
            let mut rp = p + phase_off;
            if rp >= 1.0 {
                rp -= 1.0;
            }
            let s = sample(wave, rp, dt, width, rng);
            l += s * self.pgl[i];
            r += s * self.pgr[i];
            let np = p + dt;
            self.phase[i] = if np >= 1.0 { np - 1.0 } else { np };
        }
        if self.sub_gain != 0.0 {
            let p = self.sub_phase;
            let sub = if wave == WAVE_PULSE_SUB {
                pulse(p, self.sub_dt, 0.5)
            } else {
                fast_sin(p)
            };
            l += sub * self.sub_gain;
            r += sub * self.sub_gain;
            let np = p + self.sub_dt;
            self.sub_phase = if np >= 1.0 { np - 1.0 } else { np };
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(wave: u8, hz: f32, sr: f32, n: usize) -> Vec<f32> {
        let mut o = Osc::default();
        let mut rng = Rng::new(1);
        o.configure(wave, 1, hz / sr, 0.0, 0.0, 0.0);
        (0..n).map(|_| o.tick(wave, 0.0, 0.5, &mut rng).0).collect()
    }

    #[test]
    fn every_wave_is_finite_and_bounded() {
        for wave in 0..8 {
            for hz in [55.0, 440.0, 3000.0, 12_000.0] {
                for s in run(wave, hz, 48_000.0, 4800) {
                    assert!(s.is_finite() && s.abs() < 1.6, "wave {wave} at {hz}: {s}");
                }
            }
        }
    }

    #[test]
    fn waves_have_the_right_pitch() {
        for wave in 1..7 {
            let out = run(wave, 480.0, 48_000.0, 48_000);
            let crossings = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!(
                (crossings as i32 - 480).abs() <= 2,
                "wave {wave}: {crossings}"
            );
        }
    }

    #[test]
    fn waves_have_no_dc() {
        for wave in 0..7 {
            let out = run(wave, 500.0, 48_000.0, 48_000);
            let mean = out.iter().sum::<f32>() / out.len() as f32;
            assert!(mean.abs() < 0.02, "wave {wave}: {mean}");
        }
    }

    #[test]
    fn unison_spreads_and_normalises() {
        let mut o = Osc::default();
        let mut rng = Rng::new(7);
        let g = o.configure(1, 8, 440.0 / 48_000.0, 0.3, 1.0, 0.0);
        assert!((g - 1.0 / 8f32.sqrt()).abs() < 1e-6);
        o.start(1.0, &mut rng);
        let (mut l, mut r) = (0.0, 0.0);
        for _ in 0..4800 {
            let (a, b) = o.tick(1, 0.0, 0.5, &mut rng);
            l += a * a;
            r += b * b;
        }
        assert!(l > 1.0 && r > 1.0);
    }

    #[test]
    fn no_voices_make_silence() {
        let mut o = Osc::default();
        let mut rng = Rng::new(1);
        assert_eq!(o.configure(1, 0, 0.01, 0.0, 0.0, 0.0), 0.0);
        assert_eq!(o.tick(1, 0.0, 0.5, &mut rng), (0.0, 0.0));
    }

    // ---- Measurements. ----

    /// Magnitude spectrum of `x` (a power of two long) after a Hann window, by a radix-2 FFT in f64.
    fn spectrum(x: &[f32]) -> Vec<f64> {
        let n = x.len();
        let mut re: Vec<f64> = (0..n)
            .map(|i| {
                let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
                f64::from(x[i]) * w
            })
            .collect();
        let mut im = vec![0.0f64; n];
        let mut j = 0;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
            if i < j {
                re.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let ang = -2.0 * std::f64::consts::PI / len as f64;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                    let (a, b) = (start + k, start + k + len / 2);
                    let (tr, ti) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
        (0..n / 2)
            .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
            .collect()
    }

    /// The energy of `spec` in every bin that is not within 3 bins of a multiple of `f0` (in bins), relative to the total.
    fn inharmonic_share(spec: &[f64], f0: f64) -> f64 {
        let (mut bad, mut all) = (0.0, 0.0);
        for (i, m) in spec.iter().enumerate().skip(2) {
            let h = (i as f64 / f0).round();
            let off = (i as f64 - h * f0).abs();
            all += m * m;
            if h < 1.0 || off > 3.0 {
                bad += m * m;
            }
        }
        bad / all
    }

    fn naive_saw(hz: f32, sr: f32, n: usize) -> Vec<f32> {
        let mut p = 0.0f32;
        (0..n)
            .map(|_| {
                let y = 2.0 * p - 1.0;
                p += hz / sr;
                if p >= 1.0 {
                    p -= 1.0;
                }
                y
            })
            .collect()
    }

    #[test]
    fn band_limited_waves_alias_far_less_than_naive_ones() {
        let sr = 48_000.0;
        let n = 16_384;
        // A frequency that puts a harmonic in a bin centre, so leakage does not hide the aliasing.
        for hz in [2_343.75f32, 5_859.375] {
            let f0 = f64::from(hz) / f64::from(sr) * n as f64;
            let naive = inharmonic_share(&spectrum(&naive_saw(hz, sr, n)), f0);
            let blep = inharmonic_share(&spectrum(&run(1, hz, sr, n)), f0);
            assert!(blep < naive * 0.05, "saw at {hz}: {blep} vs naive {naive}");
            for wave in [3u8, 4, 5, 6, WAVE_PWM] {
                let share = inharmonic_share(&spectrum(&run(wave, hz, sr, n)), f0);
                assert!(
                    share < naive * 0.15,
                    "wave {wave} at {hz}: {share} vs naive {naive}"
                );
            }
        }
    }

    #[test]
    fn pulse_width_sets_the_duty_cycle_and_stays_free_of_dc() {
        let sr = 48_000.0;
        for w in [0.05f32, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 0.95] {
            let mut o = Osc::default();
            let mut rng = Rng::new(1);
            o.configure(WAVE_PWM, 1, 100.0 / sr, 0.0, 0.0, 0.0);
            let out: Vec<f32> = (0..48_000)
                .map(|_| o.tick(WAVE_PWM, 0.0, w, &mut rng).0)
                .collect();
            let high = out.iter().filter(|v| **v > 0.0).count() as f32 / out.len() as f32;
            assert!((high - w).abs() < 0.01, "width {w}: duty {high}");
            let mean = out.iter().sum::<f32>() / out.len() as f32;
            assert!(mean.abs() < 0.02, "width {w}: dc {mean}");
            assert!(
                out.iter().all(|v| v.is_finite() && v.abs() < 1.3),
                "width {w}"
            );
        }
    }

    #[test]
    fn pulse_width_extremes_at_high_pitch_never_blow_up() {
        let sr = 48_000.0;
        for hz in [50.0f32, 5_000.0, 12_000.0, 20_000.0] {
            for w in [0.0f32, 0.001, 0.5, 0.999, 1.0, -3.0, 7.0] {
                let mut o = Osc::default();
                let mut rng = Rng::new(1);
                o.configure(WAVE_PWM, 1, hz / sr, 0.0, 0.0, 0.0);
                for _ in 0..4800 {
                    let (l, _) = o.tick(WAVE_PWM, 0.0, w, &mut rng);
                    assert!(l.is_finite() && l.abs() < 2.0, "{hz} Hz width {w}: {l}");
                }
            }
        }
    }

    #[test]
    fn a_swept_pulse_width_makes_no_clicks() {
        // Moving the width every sample must stay smooth: the largest step stays near the pulse's own edge size.
        let sr = 48_000.0;
        let mut o = Osc::default();
        let mut rng = Rng::new(1);
        o.configure(WAVE_PWM, 1, 220.0 / sr, 0.0, 0.0, 0.0);
        let mut prev = 0.0f32;
        let mut worst = 0.0f32;
        let mut edges = 0;
        for i in 0..96_000 {
            let w = 0.5 + 0.4 * (2.0 * std::f32::consts::PI * 3.0 * i as f32 / sr).sin();
            let (l, _) = o.tick(WAVE_PWM, 0.0, w, &mut rng);
            if i > 0 {
                let d = (l - prev).abs();
                if d > 0.6 {
                    edges += 1;
                }
                worst = worst.max(d);
            }
            prev = l;
        }
        assert!(worst < 2.2, "{worst}");
        assert!(
            edges < 96_000 / 48,
            "a hard step is only the edge itself: {edges}"
        );
    }

    #[test]
    fn the_sub_oscillator_sits_an_octave_below_at_its_level() {
        let sr = 48_000.0;
        let n = 16_384;
        let hz = 375.0f32; // bin 128 at this length; the sub lands on bin 64
        for (wave, voices) in [
            (WAVE_SAW_SUB, 1usize),
            (WAVE_SAW_SUB, 5),
            (WAVE_PULSE_SUB, 3),
        ] {
            let mut o = Osc::default();
            let mut rng = Rng::new(3);
            let norm = o.configure(wave, voices, hz / sr, 0.0, 0.0, 0.0);
            let out: Vec<f32> = (0..n)
                .map(|_| o.tick(wave, 0.0, 0.5, &mut rng).0 * norm)
                .collect();
            let spec = spectrum(&out);
            // The Hann window's gain is 0.5 and a sine's bin holds n/2 of its amplitude.
            let sub = spec[64] / (0.5 * n as f64 / 2.0);
            let expect = if wave == WAVE_PULSE_SUB {
                0.6 * 4.0 / std::f64::consts::PI
            } else {
                0.6
            };
            assert!(
                (sub - expect).abs() < 0.05,
                "wave {wave} x{voices}: sub {sub}, wanted {expect}"
            );
            // Same pitch as without the sub: the main wave's fundamental is where it was.
            assert!(spec[128] > spec[127] && spec[128] > spec[129]);
        }
        // Waves without the sub have nothing there.
        let mut o = Osc::default();
        let mut rng = Rng::new(3);
        o.configure(1, 1, hz / sr, 0.0, 0.0, 0.0);
        let out: Vec<f32> = (0..n).map(|_| o.tick(1, 0.0, 0.5, &mut rng).0).collect();
        assert!(spectrum(&out)[64] < 0.01 * spectrum(&out)[128]);
    }

    #[test]
    fn the_sub_follows_a_pitch_change_without_a_jump() {
        // The sub is what the wave-with-sub has more than the plain saw: it must stay a smooth sine under a pitch sweep.
        let sr = 48_000.0;
        let (mut with, mut plain) = (Osc::default(), Osc::default());
        let mut rng = Rng::new(1);
        let mut prev = 0.0;
        let mut worst = 0.0f32;
        for i in 0..48_000 {
            let hz = 110.0 * (1.0 + 0.2 * (i as f32 / 4_000.0).sin());
            with.configure(WAVE_SAW_SUB, 1, hz / sr, 0.0, 0.0, 0.0);
            plain.configure(1, 1, hz / sr, 0.0, 0.0, 0.0);
            let sub =
                with.tick(WAVE_SAW_SUB, 0.0, 0.5, &mut rng).0 - plain.tick(1, 0.0, 0.5, &mut rng).0;
            if i > 0 {
                worst = worst.max((sub - prev).abs());
            }
            prev = sub;
        }
        assert!(worst < 0.01, "{worst}");
    }

    fn rms_of(o: &mut Osc, wave: u8, norm: f32, n: usize) -> (f32, f32) {
        let mut rng = Rng::new(11);
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for _ in 0..n {
            let (a, b) = o.tick(wave, 0.0, 0.5, &mut rng);
            l += (a * norm).powi(2);
            r += (b * norm).powi(2);
        }
        ((l / n as f32).sqrt(), (r / n as f32).sqrt())
    }

    #[test]
    fn unison_has_the_same_power_for_every_voice_count_and_law() {
        let sr = 48_000.0;
        for wave in [1u8, WAVE_SUPERSAW] {
            let mut single = Osc::default();
            let norm = single.configure(wave, 1, 220.0 / sr, 0.0, 0.0, 0.0);
            let (reference, _) = rms_of(&mut single, wave, norm, 96_000);
            for n in 2..=MAX_UNISON {
                let mut o = Osc::default();
                let mut rng = Rng::new(5);
                let norm = o.configure(wave, n, 220.0 / sr, 0.4, 0.0, 0.0);
                o.start(1.0, &mut rng);
                let (l, _) = rms_of(&mut o, wave, norm, 96_000);
                assert!(
                    (l / reference - 1.0).abs() < 0.2,
                    "wave {wave} x{n}: {l} vs {reference}"
                );
            }
        }
    }

    #[test]
    fn the_supersaw_is_symmetric_stable_and_louder_in_the_centre() {
        let sr = 48_000.0;
        for n in [2usize, 3, 5, 7, 8] {
            let mut o = Osc::default();
            let mut rng = Rng::new(9);
            let norm = o.configure(WAVE_SUPERSAW, n, 220.0 / sr, 0.5, 0.0, 0.0);
            o.start(1.0, &mut rng);
            let (l, r) = rms_of(&mut o, WAVE_SUPERSAW, norm, 96_000);
            assert!((l / r - 1.0).abs() < 0.12, "x{n}: left {l} right {r}");
        }
        // Detune offsets mirror about the centre and grow faster than linearly.
        let mut o = Osc::default();
        o.configure(WAVE_SUPERSAW, 7, 0.01, 1.0, 0.5, 0.0);
        let r: Vec<f32> = o.ratio[..7].iter().map(|x| x.log2() * 12.0).collect();
        for i in 0..3 {
            assert!((r[i] + r[6 - i]).abs() < 1e-5, "{r:?}");
        }
        assert!(r[3].abs() < 1e-6);
        assert!(r[5] / r[6] < 0.6 && r[4] / r[6] < 0.2, "{r:?}");
        assert!(o.weight[3] > o.weight[0] && o.weight[3] > o.weight[6]);
        assert!((o.weight[0] - o.weight[6]).abs() < 1e-6);
        // The panning mirrors too.
        assert!((o.pgl[0] - o.pgr[6]).abs() < 1e-5 && (o.pgr[0] - o.pgl[6]).abs() < 1e-5);
    }

    #[test]
    fn the_new_waves_have_the_right_pitch() {
        for wave in [WAVE_PWM, WAVE_SAW_SUB, WAVE_PULSE_SUB, WAVE_SUPERSAW] {
            let out = run(wave, 480.0, 48_000.0, 48_000);
            // The sub's lower fundamental drops some crossings; what is checked is that the main pitch is a multiple.
            let crossings = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!(
                crossings > 200 && crossings <= 482,
                "wave {wave}: {crossings}"
            );
            assert!(
                out.iter().all(|v| v.is_finite() && v.abs() < 2.5),
                "wave {wave}"
            );
        }
        let out = run(WAVE_PWM, 480.0, 48_000.0, 48_000);
        let crossings = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((crossings as i32 - 480).abs() <= 2, "{crossings}");
    }

    #[test]
    fn oscillators_make_no_allocation_shaped_state() {
        // Everything is in fixed arrays: the type is plain data of a known size.
        assert!(std::mem::size_of::<Osc>() < 512);
    }
}
