// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hall: an algorithmic stereo reverb built from published, well-understood parts.
//!
//! * Input: pre-delay, then four Schroeder all-pass diffusers (Schroeder 1962; the chain and gains follow Dattorro,
//!   "Effect Design, Part 1", JAES 1997).
//! * Early reflections: a short multi-tap delay (Moorer 1979 style) from the pre-delayed input.
//! * Tail: an 8-line feedback delay network (Jot & Chaigne, AES 90, 1991; Stautner & Puckette 1982). The feedback
//!   matrix is the Householder reflection `I - 2/N 11ᵀ`, which is orthogonal, so the loop is lossless before the
//!   absorption filters. Delay lengths are mutually prime.
//! * Decay: each line carries an absorption filter whose gain is `10^(-3 m / (fs RT60))` for a line of `m` samples
//!   (Jot's relation), so the measured reverberation time follows the `decay` setting. The filter is a three-band
//!   shape: `bass_mult` stretches the decay below `bass_freq`, `hf_mult` shortens it above `hf_freq`. Its magnitude
//!   never exceeds one, so the network is stable for every setting.
//! * Modulation: each line's length is swept by a sine LFO (slightly different phase for each line) to break up the
//!   metallic ringing of a fixed network.
//!
//! Nothing here allocates after [`Hall::new`].

// The network indexes several parallel arrays by line number; iterators would obscure it.
#![allow(clippy::needless_range_loop)]

use std::f32::consts::TAU;

/// The parameters, in plain units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// 0 (dry) to 1 (wet).
    pub mix: f32,
    pub predelay_ms: f32,
    /// Reverberation time (RT60) at mid frequencies, seconds.
    pub decay_s: f32,
    /// Scales every delay length: 0.5 (small) to 2.0 (large).
    pub size: f32,
    /// 0 (instant) to 1 (slow build-up of the tail).
    pub attack: f32,
    /// Input diffusion, 0 to 1.
    pub early_diff: f32,
    /// Diffusion inside the loop, 0 to 1.
    pub late_diff: f32,
    pub hf_freq: f32,
    /// Decay above `hf_freq` as a share of the mid decay (0.05 to 1).
    pub hf_mult: f32,
    pub bass_freq: f32,
    /// Decay below `bass_freq` as a multiple of the mid decay (0.5 to 4).
    pub bass_mult: f32,
    pub mod_rate: f32,
    /// 0 to 1.
    pub mod_depth: f32,
    pub high_cut: f32,
    pub low_cut: f32,
    /// Early reflections level, 0 to 1.
    pub early: f32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            mix: 0.35,
            predelay_ms: 20.0,
            decay_s: 2.5,
            size: 1.0,
            attack: 0.3,
            early_diff: 0.8,
            late_diff: 0.8,
            hf_freq: 6000.0,
            hf_mult: 0.5,
            bass_freq: 600.0,
            bass_mult: 1.2,
            mod_rate: 0.8,
            mod_depth: 0.35,
            high_cut: 12_000.0,
            low_cut: 40.0,
            early: 0.5,
        }
    }
}

const N: usize = 8;
/// Line lengths in samples at 48 kHz (primes).
const BASE: [f32; N] = [
    1433.0, 1601.0, 1873.0, 2053.0, 2371.0, 2713.0, 3037.0, 3323.0,
];
/// Diffuser lengths at 48 kHz (primes).
const DIFFUSER: [usize; 4] = [149, 113, 397, 277];
const LOOP_AP: [usize; N] = [113, 127, 139, 149, 157, 167, 179, 191];
/// Early reflection taps: (ms, gain, side).
const TAPS: [(f32, f32, f32); 8] = [
    (4.3, 0.84, -1.0),
    (7.1, 0.72, 1.0),
    (11.3, 0.62, -1.0),
    (13.7, 0.55, 1.0),
    (17.9, 0.47, -1.0),
    (23.3, 0.40, 1.0),
    (29.1, 0.33, -1.0),
    (34.7, 0.27, 1.0),
];
const L_SIGNS: [f32; N] = [1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0];
const R_SIGNS: [f32; N] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const MAX_PREDELAY: f32 = 0.5;
const BLOCK: usize = 32;

/// A Schroeder all-pass: unit gain at every frequency.
#[derive(Clone)]
struct Allpass {
    buf: Vec<f32>,
    i: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Allpass {
            buf: vec![0.0; len.max(2)],
            i: 0,
        }
    }
    #[inline]
    fn run(&mut self, x: f32, g: f32) -> f32 {
        let b = self.buf[self.i];
        let w = x + g * b;
        self.buf[self.i] = w;
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
        b - g * w
    }
    fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

#[derive(Clone)]
struct Line {
    buf: Vec<f32>,
    w: usize,
    /// The current length in samples (glides to the target).
    len: f32,
    hi: f32,
    lo: f32,
    ap: Allpass,
}

impl Line {
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let n = self.buf.len();
        let d = delay.clamp(2.0, (n - 2) as f32);
        let whole = d as usize;
        let frac = d - whole as f32;
        let a = self.buf[(self.w + n - whole) % n];
        let b = self.buf[(self.w + n - whole - 1) % n];
        a + (b - a) * frac
    }
}

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

/// One-pole coefficient with the -3 dB point at `f`.
fn pole(f: f32, sr: f32) -> f32 {
    (-TAU * f.clamp(5.0, sr * 0.45) / sr).exp()
}

pub struct Hall {
    sr: f32,
    p: Params,
    pre: Vec<f32>,
    pre_w: usize,
    diff: [Allpass; 4],
    lines: Vec<Line>,
    phase: [f32; N],
    attack_lp: f32,
    er_buf: Vec<f32>,
    er_w: usize,
    hc: [f32; 2],
    lc: [f32; 2],
    // Per-block derived values.
    g_mid: [f32; N],
    g_bass: [f32; N],
    a_hi: f32,
    a_lo: f32,
    d_hi: f32,
    ap_g: f32,
    diff_g: f32,
    target: [f32; N],
    mod_samples: f32,
    inc: f32,
    a_attack: f32,
    a_hc: f32,
    a_lc: f32,
    pre_samples: f32,
}

impl Hall {
    pub fn new(sr: f32) -> Self {
        let sr = sr.clamp(8_000.0, 384_000.0);
        let k = sr / 48_000.0;
        let max_line = (BASE[N - 1] * 2.0 * k) as usize + 64;
        let lines = (0..N)
            .map(|i| Line {
                buf: vec![0.0; max_line],
                w: 0,
                len: BASE[i] * k,
                hi: 0.0,
                lo: 0.0,
                ap: Allpass::new((LOOP_AP[i] as f32 * k) as usize),
            })
            .collect();
        let mut h = Hall {
            sr,
            p: Params::default(),
            pre: vec![0.0; (MAX_PREDELAY * sr) as usize + 4],
            pre_w: 0,
            diff: DIFFUSER.map(|n| Allpass::new((n as f32 * k) as usize)),
            lines,
            phase: std::array::from_fn(|i| i as f32 / N as f32),
            attack_lp: 0.0,
            er_buf: vec![0.0; (0.06 * sr) as usize + 4],
            er_w: 0,
            hc: [0.0; 2],
            lc: [0.0; 2],
            g_mid: [0.0; N],
            g_bass: [0.0; N],
            a_hi: 0.0,
            a_lo: 0.0,
            d_hi: 0.0,
            ap_g: 0.0,
            diff_g: 0.0,
            target: [0.0; N],
            mod_samples: 0.0,
            inc: 0.0,
            a_attack: 0.0,
            a_hc: 0.0,
            a_lc: 0.0,
            pre_samples: 0.0,
        };
        h.set(&Params::default());
        h
    }

    pub fn reset(&mut self) {
        self.pre.fill(0.0);
        self.er_buf.fill(0.0);
        self.diff.iter_mut().for_each(Allpass::clear);
        for l in &mut self.lines {
            l.buf.fill(0.0);
            l.hi = 0.0;
            l.lo = 0.0;
            l.ap.clear();
        }
        self.attack_lp = 0.0;
        self.hc = [0.0; 2];
        self.lc = [0.0; 2];
    }

    /// Takes new parameters (cheap: derived values are recomputed here, not per sample).
    pub fn set(&mut self, p: &Params) {
        let sr = self.sr;
        let k = sr / 48_000.0;
        self.p = Params {
            mix: p.mix.clamp(0.0, 1.0),
            predelay_ms: p.predelay_ms.clamp(0.0, MAX_PREDELAY * 1000.0),
            decay_s: p.decay_s.clamp(0.1, 60.0),
            size: p.size.clamp(0.5, 2.0),
            attack: p.attack.clamp(0.0, 1.0),
            early_diff: p.early_diff.clamp(0.0, 1.0),
            late_diff: p.late_diff.clamp(0.0, 1.0),
            hf_freq: p.hf_freq.clamp(200.0, 20_000.0),
            hf_mult: p.hf_mult.clamp(0.05, 1.0),
            bass_freq: p.bass_freq.clamp(40.0, 2_000.0),
            bass_mult: p.bass_mult.clamp(0.5, 4.0),
            mod_rate: p.mod_rate.clamp(0.0, 10.0),
            mod_depth: p.mod_depth.clamp(0.0, 1.0),
            high_cut: p.high_cut.clamp(200.0, 20_000.0),
            low_cut: p.low_cut.clamp(10.0, 2_000.0),
            early: p.early.clamp(0.0, 1.0),
        };
        let p = self.p;
        for i in 0..N {
            self.target[i] = BASE[i] * k * p.size;
        }
        self.a_hi = pole(p.hf_freq, sr);
        self.a_lo = pole(p.bass_freq, sr);
        self.d_hi = 1.0 - p.hf_mult;
        self.ap_g = 0.6 * p.late_diff;
        self.diff_g = 0.75 * p.early_diff;
        self.mod_samples = p.mod_depth * 12.0 * k;
        self.inc = p.mod_rate / sr;
        self.a_attack = if p.attack <= 0.0 {
            0.0
        } else {
            (-1.0 / (p.attack * 0.15 * sr)).exp()
        };
        self.a_hc = pole(p.high_cut, sr);
        self.a_lc = pole(p.low_cut, sr);
        self.pre_samples = p.predelay_ms * 0.001 * sr;
    }

    /// The per-line decay gains for the current lengths (Jot's relation).
    fn gains(&mut self) {
        let p = self.p;
        for i in 0..N {
            let m = self.lines[i].len;
            let per = |rt: f32| 10f32.powf(-3.0 * m / (self.sr * rt));
            self.g_mid[i] = per(p.decay_s);
            self.g_bass[i] = per(p.decay_s * p.bass_mult).min(0.99999);
        }
    }

    /// Reverberates `l`/`r` in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut at = 0;
        while at < n {
            let end = (at + BLOCK).min(n);
            // Glide the lengths toward their targets, then refresh the gains.
            for i in 0..N {
                let line = &mut self.lines[i];
                line.len += (self.target[i] - line.len) * 0.02;
            }
            self.gains();
            for s in at..end {
                let (dl, dr) = (l[s], r[s]);
                let x = 0.5 * (dl + dr);
                // Pre-delay.
                let pn = self.pre.len();
                self.pre[self.pre_w] = x;
                let d = self.pre_samples.clamp(0.0, (pn - 2) as f32);
                let whole = d as usize;
                let frac = d - whole as f32;
                let a = self.pre[(self.pre_w + pn - whole) % pn];
                let b = self.pre[(self.pre_w + pn - whole - 1) % pn];
                let xp = a + (b - a) * frac;
                self.pre_w = (self.pre_w + 1) % pn;
                // Early reflections.
                let en = self.er_buf.len();
                self.er_buf[self.er_w] = xp;
                let (mut el, mut er) = (0.0, 0.0);
                for (ms, g, side) in TAPS {
                    let t = ((ms * 0.001 * self.sr * self.p.size.sqrt()) as usize).min(en - 1);
                    let v = self.er_buf[(self.er_w + en - t) % en] * g;
                    if side < 0.0 {
                        el += v;
                    } else {
                        er += v;
                    }
                }
                self.er_w = (self.er_w + 1) % en;
                // Input diffusion.
                let mut inp = xp;
                for ap in &mut self.diff {
                    inp = ap.run(inp, self.diff_g);
                }
                // Attack: a one-pole that softens the onset of what feeds the tail.
                self.attack_lp = flush(inp + self.a_attack * (self.attack_lp - inp));
                let feed = if self.a_attack > 0.0 {
                    self.attack_lp
                } else {
                    inp
                };
                // The network.
                let mut y = [0.0f32; N];
                for i in 0..N {
                    let line = &mut self.lines[i];
                    let lfo = (TAU * self.phase[i]).sin();
                    let delay = line.len + self.mod_samples * lfo;
                    y[i] = line.read(delay);
                }
                // Absorption (per line) and the matrix.
                let mut sum = 0.0;
                for i in 0..N {
                    let line = &mut self.lines[i];
                    let d = y[i];
                    // Two unity-DC-gain low-passes of the delayed sample (y = (1-a) x + a y).
                    line.hi = flush((1.0 - self.a_hi) * d + self.a_hi * line.hi);
                    line.lo = flush((1.0 - self.a_lo) * d + self.a_lo * line.lo);
                    let gm = self.g_mid[i];
                    let v = gm * ((1.0 - self.d_hi) * d + self.d_hi * line.hi)
                        + (self.g_bass[i] - gm) * line.lo;
                    y[i] = v;
                    sum += v;
                }
                let s2 = sum * (2.0 / N as f32);
                let (mut wl, mut wr) = (0.0, 0.0);
                for i in 0..N {
                    wl += L_SIGNS[i] * y[i];
                    wr += R_SIGNS[i] * y[i];
                    let fb = y[i] - s2;
                    let inj = if i % 2 == 0 { feed } else { -feed } * 0.35;
                    let line = &mut self.lines[i];
                    let v = line.ap.run(fb + inj, self.ap_g);
                    let w = line.w;
                    line.buf[w] = flush(v);
                    line.w = (w + 1) % line.buf.len();
                    self.phase[i] += self.inc;
                    if self.phase[i] >= 1.0 {
                        self.phase[i] -= 1.0;
                    }
                }
                let norm = 0.5 / (N as f32).sqrt() * 2.0;
                let mut wet = [
                    wl * norm + el * self.p.early * 0.5,
                    wr * norm + er * self.p.early * 0.5,
                ];
                // Tone: low cut then high cut on the wet signal.
                for (c, w) in wet.iter_mut().enumerate() {
                    self.lc[c] = flush((1.0 - self.a_lc) * *w + self.a_lc * self.lc[c]);
                    let hp = *w - self.lc[c];
                    self.hc[c] = flush((1.0 - self.a_hc) * hp + self.a_hc * self.hc[c]);
                    *w = self.hc[c];
                }
                let m = self.p.mix;
                l[s] = dl * (1.0 - m) + wet[0] * m;
                r[s] = dr * (1.0 - m) + wet[1] * m;
            }
            at = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Parameters with everything that colours the measurement switched off.
    fn plain(decay: f32) -> Params {
        Params {
            mix: 1.0,
            predelay_ms: 0.0,
            decay_s: decay,
            size: 1.0,
            attack: 0.0,
            early_diff: 0.8,
            late_diff: 0.8,
            hf_freq: 6000.0,
            hf_mult: 1.0,
            bass_freq: 600.0,
            bass_mult: 1.0,
            mod_rate: 0.0,
            mod_depth: 0.0,
            high_cut: 20_000.0,
            low_cut: 10.0,
            early: 0.0,
        }
    }

    fn impulse(p: &Params, secs: f32) -> (Vec<f32>, Vec<f32>) {
        let mut h = Hall::new(SR);
        h.set(p);
        let n = (secs * SR) as usize;
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        l[0] = 1.0;
        r[0] = 1.0;
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            h.process(cl, cr);
        }
        (l, r)
    }

    /// RT60 from Schroeder's backward integral, fitted between -5 and -25 dB and extrapolated (ISO 3382 T20).
    fn rt60(x: &[f32]) -> f32 {
        let mut e = vec![0.0f64; x.len()];
        let mut acc = 0.0f64;
        for i in (0..x.len()).rev() {
            acc += (x[i] as f64).powi(2);
            e[i] = acc;
        }
        let db: Vec<f64> = e
            .iter()
            .map(|v| 10.0 * (v / e[0]).max(1e-30).log10())
            .collect();
        let at = |t: f64| db.iter().position(|&v| v <= t).unwrap();
        let (a, b) = (at(-5.0), at(-25.0));
        3.0 * (b - a) as f32 / SR
    }

    fn lowpass(x: &[f32], f: f32) -> Vec<f32> {
        let a = pole(f, SR);
        let mut s = 0.0;
        x.iter()
            .map(|&v| {
                s = (1.0 - a) * v + a * s;
                s
            })
            .collect()
    }

    #[test]
    fn the_reverberation_time_follows_the_decay_setting() {
        for decay in [1.0f32, 2.5, 5.0] {
            let (l, _) = impulse(&plain(decay), decay * 1.6 + 1.0);
            let t = rt60(&l);
            assert!(
                (t / decay - 1.0).abs() < 0.2,
                "decay {decay} s measured {t:.2} s"
            );
        }
    }

    #[test]
    fn a_high_damping_shortens_the_treble_but_not_the_bass() {
        let p = Params {
            hf_mult: 0.25,
            ..plain(4.0)
        };
        let (l, _) = impulse(&p, 8.0);
        let low = rt60(&lowpass(&lowpass(&l, 400.0), 400.0));
        let high: Vec<f32> = {
            let lp = lowpass(&lowpass(&l, 5000.0), 5000.0);
            l.iter().zip(&lp).map(|(a, b)| a - b).collect()
        };
        let high = rt60(&high);
        assert!(low > 3.0, "bass band {low:.2} s");
        assert!(high < 0.6 * low, "bass {low:.2} s, treble {high:.2} s");
    }

    #[test]
    fn a_bass_multiplier_lengthens_the_low_decay() {
        let flat = impulse(&plain(2.0), 8.0).0;
        let boosted = impulse(
            &Params {
                bass_mult: 2.0,
                ..plain(2.0)
            },
            8.0,
        )
        .0;
        let band = |x: &[f32]| rt60(&lowpass(&lowpass(x, 120.0), 120.0));
        assert!(
            band(&boosted) > 1.4 * band(&flat),
            "{} {}",
            band(&boosted),
            band(&flat)
        );
    }

    #[test]
    fn the_householder_matrix_is_orthogonal() {
        // A = I - 2/N 1 1^T applied to the unit vectors: columns have unit length and are mutually orthogonal.
        let col = |j: usize| -> Vec<f32> {
            (0..N)
                .map(|i| (i == j) as u8 as f32 - 2.0 / N as f32)
                .collect()
        };
        for a in 0..N {
            for b in 0..N {
                let dot: f32 = col(a).iter().zip(col(b)).map(|(x, y)| x * y).sum();
                let want = (a == b) as u8 as f32;
                assert!((dot - want).abs() < 1e-6, "{a} {b} {dot}");
            }
        }
    }

    #[test]
    fn an_allpass_keeps_the_energy_of_an_impulse() {
        let mut ap = Allpass::new(113);
        let e: f32 = (0..4000)
            .map(|i| ap.run(if i == 0 { 1.0 } else { 0.0 }, 0.7).powi(2))
            .sum();
        assert!((e - 1.0).abs() < 1e-3, "{e}");
    }

    #[test]
    fn every_setting_is_stable_and_finite() {
        // A deterministic sweep of corners and a pseudo-random walk through the parameter space.
        let mut seed = 0x1234_5678u32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..40 {
            let p = Params {
                mix: rnd(),
                predelay_ms: rnd() * 500.0,
                decay_s: 0.1 + rnd() * 59.0,
                size: 0.5 + rnd() * 1.5,
                attack: rnd(),
                early_diff: rnd(),
                late_diff: rnd(),
                hf_freq: 200.0 + rnd() * 19_000.0,
                hf_mult: 0.05 + rnd() * 0.95,
                bass_freq: 40.0 + rnd() * 1960.0,
                bass_mult: 0.5 + rnd() * 3.5,
                mod_rate: rnd() * 10.0,
                mod_depth: rnd(),
                high_cut: 200.0 + rnd() * 19_800.0,
                low_cut: 10.0 + rnd() * 1990.0,
                early: rnd(),
            };
            let mut h = Hall::new(SR);
            h.set(&p);
            let mut peak = 0.0f32;
            for blk in 0..300 {
                let mut l: Vec<f32> = (0..128)
                    .map(|i| if blk < 4 { rnd() * 2.0 - 1.0 } else { 0.0 } * (i as f32 / 128.0))
                    .collect();
                let mut r = l.clone();
                h.process(&mut l, &mut r);
                for v in l.iter().chain(&r) {
                    assert!(v.is_finite(), "{p:?}");
                    peak = peak.max(v.abs());
                }
            }
            assert!(peak < 20.0, "{peak} for {p:?}");
        }
    }

    #[test]
    fn the_tail_dies_away_and_dry_passes_at_zero_mix() {
        let (l, _) = impulse(&plain(1.0), 6.0);
        let tail: f32 = l[(5.0 * SR) as usize..]
            .iter()
            .map(|v| v.abs())
            .fold(0.0, f32::max);
        assert!(tail < 1e-3, "{tail}");
        let mut h = Hall::new(SR);
        h.set(&Params {
            mix: 0.0,
            ..Params::default()
        });
        let mut a: Vec<f32> = (0..500).map(|i| (i as f32 * 0.05).sin()).collect();
        let mut b = a.clone();
        let want = a.clone();
        h.process(&mut a, &mut b);
        assert_eq!(a, want);
    }

    #[test]
    fn a_pre_delay_holds_the_wet_signal_back() {
        let p = Params {
            predelay_ms: 50.0,
            ..plain(2.0)
        };
        let (l, _) = impulse(&p, 0.5);
        let first = l.iter().position(|v| v.abs() > 1e-4).unwrap();
        assert!(first as f32 / SR >= 0.049, "first wet sample at {first}");
    }
}
