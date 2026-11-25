// SPDX-License-Identifier: GPL-3.0-or-later
// The sound of Rind Tape: a tape machine after Jatin Chowdhury's ChowTape (AnalogTapeModel, GPL-3.0-or-later,
// https://github.com/jatinchowdhury18/AnalogTapeModel): the Jiles-Atherton hysteresis of the tape (solved by four Newton-Raphson
// steps per sample, at twice the sample rate) and the loss of the playback head (spacing, thickness and gap, as a linear-phase
// filter, with the head bump). Wow and flutter are a modulated delay of Honeydew's own. Plain Rust, no dependencies.
use std::f64::consts::{PI, TAU};

/// The playback head's filter: its taps (a power of two keeps its delay a whole number).
pub const LOSS_TAPS: usize = 64;
/// The oversampling filter's taps, at twice the sample rate.
const OS_TAPS: usize = 31;
/// The wow/flutter delay line's rest delay, in samples.
pub const BASE_DELAY: usize = 64;
/// The most the delay line swings either way, in samples.
const SWING: f64 = 60.;
/// What the whole sound delays by, in samples: the oversampling, the loss filter and the delay line.
pub const LATENCY: u32 = ((OS_TAPS - 1) / 2 / 2 + LOSS_TAPS / 2 + BASE_DELAY) as u32;

const ALPHA: f64 = 1.6e-3;

/// The settings the sound is made from (what the editor's knobs mean, in the units of the tape).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tape {
    /// 0..1
    pub drive: f64,
    /// 0..1
    pub sat: f64,
    /// 0..1: ChowTape's "bias".
    pub bias: f64,
    /// Inches per second.
    pub speed: f64,
    /// Microns.
    pub spacing: f64,
    /// Microns.
    pub gap: f64,
    /// 0..1
    pub wow: f64,
    /// 0..1
    pub flutter: f64,
    /// 0..1
    pub mix: f64,
    /// Linear gain.
    pub out: f64,
}
impl Default for Tape {
    fn default() -> Self {
        Self {
            drive: 0.5,
            sat: 0.5,
            bias: 0.5,
            speed: 15.,
            spacing: 1.4,
            gap: 1.,
            wow: 0.,
            flutter: 0.,
            mix: 1.,
            out: 1.,
        }
    }
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// A windowed-sinc low-pass of `OS_TAPS` taps cutting at a quarter of the oversampled rate, unity gain at DC.
fn halfband() -> [f64; OS_TAPS] {
    let mut h = [0.; OS_TAPS];
    let mid = (OS_TAPS - 1) as f64 / 2.;
    let mut sum = 0.;
    for (n, t) in h.iter_mut().enumerate() {
        let x = n as f64 - mid;
        let w = 0.42 - 0.5 * (TAU * n as f64 / (OS_TAPS - 1) as f64).cos()
            + 0.08 * (2. * TAU * n as f64 / (OS_TAPS - 1) as f64).cos();
        *t = 0.5 * sinc(0.5 * x) * w;
        sum += *t;
    }
    for t in &mut h {
        *t /= sum;
    }
    h
}

/// A history of the last `N` samples.
#[derive(Clone)]
struct Line<const N: usize> {
    buf: [f64; N],
    at: usize,
}
impl<const N: usize> Line<N> {
    fn new() -> Self {
        Self {
            buf: [0.; N],
            at: 0,
        }
    }
    fn push(&mut self, x: f64) {
        self.at = (self.at + 1) % N;
        self.buf[self.at] = x;
    }
    /// The sum of the taps times the samples, the newest first.
    fn dot(&self, taps: &[f64; N]) -> f64 {
        let mut s = 0.;
        let mut i = self.at;
        for t in taps {
            s += t * self.buf[i];
            i = if i == 0 { N - 1 } else { i - 1 };
        }
        s
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}
impl Biquad {
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
    fn highpass(rate: f64, freq: f64, q: f64) -> Self {
        let w = TAU * freq / rate;
        let (s, c) = w.sin_cos();
        let al = s / (2. * q);
        let a0 = 1. + al;
        Self {
            b: [(1. + c) / 2. / a0, -(1. + c) / a0, (1. + c) / 2. / a0],
            a: [-2. * c / a0, (1. - al) / a0],
            z: [0.; 2],
        }
    }
    /// A peak of linear amplitude `gain` at `freq`.
    fn peak(rate: f64, freq: f64, q: f64, gain: f64) -> Self {
        let a = gain.max(1e-3).sqrt();
        let w = TAU * freq / rate;
        let (s, c) = w.sin_cos();
        let al = s / (2. * q);
        let a0 = 1. + al / a;
        Self {
            b: [(1. + al * a) / a0, -2. * c / a0, (1. - al * a) / a0],
            a: [-2. * c / a0, (1. - al / a) / a0],
            z: [0.; 2],
        }
    }
}

/// The tape's magnetisation (Jiles-Atherton), one channel.
#[derive(Clone, Default)]
struct Hysteresis {
    t: f64,
    talpha: f64,
    m_s: f64,
    a: f64,
    c: f64,
    k: f64,
    nc: f64,
    m_s_oa: f64,
    m_s_oa_talpha: f64,
    m_s_oa_tc: f64,
    m_s_oa_tc_talpha: f64,
    m_s_oa_sq_tc_talpha: f64,
    m_s_oa_sq_tc_talpha_sq: f64,
    m1: f64,
    h1: f64,
    hd1: f64,
}
/// What `Hysteresis::func` works out and `prime` reuses.
struct Cache {
    q: f64,
    one_over_q: f64,
    near_zero: bool,
    coth: f64,
    m_diff: f64,
    kap1: f64,
    l_prime: f64,
    one_over_f1: f64,
    one_over_f3: f64,
}
impl Hysteresis {
    fn new(rate: f64) -> Self {
        let mut h = Self {
            t: 1. / rate,
            talpha: 1. / rate / 1.9,
            ..Self::default()
        };
        h.cook(0.5, 0.5, 0.5);
        h
    }
    fn cook(&mut self, drive: f64, width: f64, sat: f64) {
        self.m_s = 0.5 + 1.5 * (1. - sat);
        self.a = self.m_s / (0.01 + 6. * drive);
        self.c = (1. - width).sqrt() - 0.01;
        self.k = 0.47875;
        self.nc = 1. - self.c;
        self.m_s_oa = self.m_s / self.a;
        self.m_s_oa_talpha = ALPHA * self.m_s_oa;
        self.m_s_oa_tc = self.c * self.m_s_oa;
        self.m_s_oa_tc_talpha = ALPHA * self.m_s_oa_tc;
        self.m_s_oa_sq_tc_talpha = self.m_s_oa_tc_talpha / self.a;
        self.m_s_oa_sq_tc_talpha_sq = ALPHA * self.m_s_oa_sq_tc_talpha;
    }
    /// dM/dt.
    fn func(&self, m: f64, h: f64, hd: f64) -> (f64, Cache) {
        let q = (h + m * ALPHA) / self.a;
        let one_over_q = 1. / q;
        let coth = 1. / q.tanh();
        let near_zero = q < 0.001 && q > -0.001;
        let langevin = if near_zero { q / 3. } else { coth - one_over_q };
        let m_diff = langevin * self.m_s - m;
        let delta = if hd >= 0. { 1. } else { -1. };
        let sign = |x: f64| i32::from(x > 0.) - i32::from(x < 0.);
        let kap1 = if sign(delta) == sign(m_diff) {
            self.nc
        } else {
            0.
        };
        let l_prime = if near_zero {
            1. / 3.
        } else {
            one_over_q * one_over_q - coth * coth + 1.
        };
        let f1_denom = self.nc * delta * self.k - ALPHA * m_diff;
        let f1 = kap1 * m_diff / f1_denom;
        let f2 = l_prime * self.m_s_oa_tc;
        let f3 = 1. - l_prime * self.m_s_oa_tc_talpha;
        let cache = Cache {
            q,
            one_over_q,
            near_zero,
            coth,
            m_diff,
            kap1,
            l_prime,
            one_over_f1: 1. / f1_denom,
            one_over_f3: 1. / f3,
        };
        (hd * (f1 + f2) / f3, cache)
    }
    /// d(dM/dt)/dM, from what `func` cached.
    fn prime(&self, hd: f64, dmdt: f64, c: &Cache) -> f64 {
        let l_prime2 = if c.near_zero {
            -2. / 15. * c.q
        } else {
            2. * c.coth * (c.coth * c.coth - 1.) - 2. * c.one_over_q * c.one_over_q * c.one_over_q
        };
        let m_diff2 = c.l_prime * self.m_s_oa_talpha - 1.;
        let mut f1_p = m_diff2 * c.one_over_f1;
        f1_p += c.m_diff * ALPHA * m_diff2 * c.one_over_f1 * c.one_over_f1;
        f1_p *= c.kap1;
        let f2_p = l_prime2 * self.m_s_oa_sq_tc_talpha;
        let f3_p = l_prime2 * -self.m_s_oa_sq_tc_talpha_sq;
        (hd * (f1_p + f2_p) - dmdt * f3_p) * c.one_over_f3
    }
    fn process(&mut self, h: f64) -> f64 {
        let mut hd = (1.75 / self.t) * (h - self.h1) - 0.75 * self.hd1;
        let last = self.func(self.m1, self.h1, self.hd1).0;
        let mut m = self.m1;
        for _ in 0..4 {
            let (dmdt, cache) = self.func(m, h, hd);
            let dprime = self.prime(hd, dmdt, &cache);
            m -= (m - self.m1 - self.talpha * (dmdt + last)) / (1. - self.talpha * dprime);
        }
        if m.is_nan() || m > 20. {
            m = 0.;
            hd = 0.;
        }
        self.m1 = m;
        self.h1 = h;
        self.hd1 = hd;
        m
    }
}

/// One channel of the machine.
#[derive(Clone)]
struct Channel {
    hyst: Hysteresis,
    up: Line<OS_TAPS>,
    down: Line<OS_TAPS>,
    dc: [Biquad; 2],
    loss: Line<LOSS_TAPS>,
    bump: Biquad,
    delay: Vec<f64>,
    at: usize,
    dry: Vec<f64>,
    dry_at: usize,
}
impl Channel {
    fn new(rate: f64) -> Self {
        Self {
            hyst: Hysteresis::new(rate * 2.),
            up: Line::new(),
            down: Line::new(),
            dc: [
                Biquad::highpass(rate, 35., 0.5412),
                Biquad::highpass(rate, 35., 1.3066),
            ],
            loss: Line::new(),
            bump: Biquad::peak(rate, 100., 2., 1.),
            delay: vec![0.; 2048],
            at: 0,
            dry: vec![0.; LATENCY as usize + 1],
            dry_at: 0,
        }
    }
}

/// A delay line read at a fractional distance (cubic Hermite).
fn read(buf: &[f64], at: usize, d: f64) -> f64 {
    let n = buf.len();
    let i = d.floor();
    let f = d - i;
    let i = i as usize;
    let at = |k: usize| buf[(at + n - (k % n)) % n];
    let (y0, y1, y2, y3) = (at(i.saturating_sub(1)), at(i), at(i + 1), at(i + 2));
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2. * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * f + c2) * f + c1) * f + y1
}

pub struct Machine {
    rate: f64,
    now: Tape,
    set: Tape,
    ch: [Channel; 2],
    half: [f64; OS_TAPS],
    taps: [f64; LOSS_TAPS],
    taps_for: Option<(f64, f64, f64)>,
    cos: Vec<f64>,
    wow_phase: f64,
    flutter_phase: f64,
    drift: u32,
    drift_to: f64,
    drift_now: f64,
    /// Until the first `set`, which also becomes where the settings glide from.
    fresh: bool,
}
impl Machine {
    pub fn new(rate: f64) -> Self {
        let rate = rate.clamp(8000., 192000.);
        let cos = (0..LOSS_TAPS)
            .map(|i| (TAU * i as f64 / LOSS_TAPS as f64).cos())
            .collect();
        let mut m = Self {
            rate,
            now: Tape::default(),
            set: Tape::default(),
            ch: [Channel::new(rate), Channel::new(rate)],
            half: halfband(),
            taps: [0.; LOSS_TAPS],
            taps_for: None,
            cos,
            wow_phase: 0.,
            flutter_phase: 0.,
            drift: 0x2545_f491,
            drift_to: 1.,
            drift_now: 1.,
            fresh: true,
        };
        m.taps[LOSS_TAPS / 2] = 1.;
        m
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.rate).with(self.set);
    }
    fn with(mut self, t: Tape) -> Self {
        self.set = t;
        self.now = t;
        self.fresh = false;
        self
    }
    pub fn set(&mut self, t: Tape) {
        self.set = t;
        if self.fresh {
            self.now = t;
            self.fresh = false;
        }
    }
    /// ChowTape's loss filter: the head's frequency response, as the taps of a linear-phase filter.
    fn design(&mut self, speed: f64, spacing: f64, gap: f64) {
        let n = LOSS_TAPS;
        let bin = self.rate / n as f64;
        let thick = 2.;
        let mut hf = [0.; LOSS_TAPS];
        for k in 0..n / 2 {
            let freq = k as f64 * bin;
            let wave = TAU * freq.max(20.) / (speed * 0.0254);
            let thick_k = wave * thick * 1e-6;
            let gap_over_two = wave * gap * 1e-6 / 2.;
            let mut v = (-wave * spacing * 1e-6).exp();
            v *= (1. - (-thick_k).exp()) / thick_k;
            v *= gap_over_two.sin() / gap_over_two;
            hf[k] = v;
            hf[n - k - 1] = v;
        }
        let mut h = [0.; LOSS_TAPS];
        for i in 0..n / 2 {
            let idx = n / 2 + i;
            let mut s = 0.;
            for (k, v) in hf.iter().enumerate() {
                s += v * self.cos[(k * i) % n];
            }
            h[idx] = s / n as f64;
            h[n / 2 - i] = h[idx];
        }
        self.taps = h;
        let bump = speed * 0.0254 / (gap * 1e-6 * 500.);
        let gain = (1.5 * (1000. - (bump - 100.).abs()) / 1000.).max(1.);
        let f = bump.clamp(20., self.rate * 0.45);
        for c in &mut self.ch {
            let z = c.bump.z;
            c.bump = Biquad::peak(self.rate, f, 2., gain);
            c.bump.z = z;
        }
    }
    /// Processes a block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        if n == 0 {
            return;
        }
        // Settings glide from block to block.
        let g = 0.3;
        let (s, w) = (self.set, &mut self.now);
        let mv = |a: &mut f64, b: f64| *a += (b - *a) * g;
        mv(&mut w.drive, s.drive);
        mv(&mut w.sat, s.sat);
        mv(&mut w.bias, s.bias);
        mv(&mut w.speed, s.speed);
        mv(&mut w.spacing, s.spacing);
        mv(&mut w.gap, s.gap);
        mv(&mut w.wow, s.wow);
        mv(&mut w.flutter, s.flutter);
        mv(&mut w.mix, s.mix);
        mv(&mut w.out, s.out);
        let now = self.now;
        let key = (now.speed, now.spacing, now.gap);
        if self.taps_for.is_none_or(|k| {
            (k.0 - key.0).abs() > 1e-3 * k.0
                || (k.1 - key.1).abs() > 1e-3 * k.1
                || (k.2 - key.2).abs() > 1e-3 * k.2
        }) {
            self.design(key.0, key.1, key.2);
            self.taps_for = Some(key);
        }
        let width = 1. - now.bias;
        let makeup = (1. + 0.6 * width) / (0.5 + 1.5 * (1. - now.sat));
        for c in &mut self.ch {
            c.hyst.cook(now.drive, width, now.sat);
        }
        let wow_hz = 0.9;
        let flutter_hz = 7.;
        for i in 0..n {
            // A slow random walk keeps the wow from sounding like a tremolo.
            if self.drift == 0 || i % 4096 == 0 {
                self.drift = self.drift.wrapping_mul(1664525).wrapping_add(1013904223);
                self.drift_to = 0.8 + 0.4 * f64::from(self.drift >> 8) / f64::from(1u32 << 24);
            }
            self.drift_now += (self.drift_to - self.drift_now) * 1e-4;
            self.wow_phase = (self.wow_phase + TAU * wow_hz * self.drift_now / self.rate) % TAU;
            self.flutter_phase = (self.flutter_phase + TAU * flutter_hz / self.rate) % TAU;
            let wow = self.wow_phase.sin();
            let fl = self.flutter_phase.sin()
                + 0.4 * (2. * self.flutter_phase).sin()
                + 0.25 * (3. * self.flutter_phase).sin();
            // Wow swings up to 1.2 ms either way, flutter up to 0.15 ms (times its harmonics).
            let swing = ((now.wow * wow * 1.2 + now.flutter * fl * 0.15) * self.rate / 1000.)
                .clamp(-SWING, SWING);
            for (k, buf) in [&mut l[i], &mut r[i]].into_iter().enumerate() {
                let c = &mut self.ch[k];
                let x = if buf.is_finite() { f64::from(*buf) } else { 0. };
                // Dry, delayed like the wet.
                c.dry[c.dry_at] = x;
                let dry = c.dry[(c.dry_at + 1) % c.dry.len()];
                c.dry_at = (c.dry_at + 1) % c.dry.len();
                // Twice the rate: up, hysteresis, down.
                let mut out = 0.;
                for j in 0..2 {
                    c.up.push(if j == 0 { 2. * x } else { 0. });
                    let u = c.up.dot(&self.half);
                    let m = c.hyst.process(u);
                    c.down.push(m);
                    if j == 1 {
                        out = c.down.dot(&self.half);
                    }
                }
                let mut y = out * makeup;
                y = c.dc[0].tick(y);
                y = c.dc[1].tick(y);
                c.loss.push(y);
                y = c.loss.dot(&self.taps);
                y = c.bump.tick(y);
                c.delay[c.at] = y;
                let d = BASE_DELAY as f64 + swing;
                let wet = read(&c.delay, c.at, d);
                c.at = (c.at + 1) % c.delay.len();
                let o = (dry * (1. - now.mix) + wet * now.mix) * now.out;
                *buf = if o.is_finite() {
                    o.clamp(-64., 64.) as f32
                } else {
                    0.
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize, hz: f64, amp: f64, rate: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (amp * (TAU * hz * i as f64 / rate).sin()) as f32)
            .collect()
    }
    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn it_stays_finite_and_makes_sound() {
        for rate in [22050., 44100., 48000., 96000.] {
            let mut m = Machine::new(rate);
            let mut l = sine(8192, 220., 0.8, rate);
            let mut r = l.clone();
            for chunk in 0..8 {
                let a = chunk * 1024;
                m.process(&mut l[a..a + 1024], &mut r[a..a + 1024]);
            }
            assert!(
                l.iter().chain(&r).all(|x| x.is_finite() && x.abs() < 8.),
                "{rate}"
            );
            assert!(rms(&l[4096..]) > 0.05, "{rate}: silent");
        }
    }

    #[test]
    fn the_extremes_stay_finite() {
        let mut m = Machine::new(48000.);
        m.set(Tape {
            drive: 1.,
            sat: 1.,
            bias: 0.,
            speed: 1.,
            spacing: 20.,
            gap: 50.,
            wow: 1.,
            flutter: 1.,
            mix: 1.,
            out: 4.,
        });
        let mut l = sine(16384, 90., 1., 48000.);
        let mut r: Vec<f32> = (0..16384)
            .map(|i| if i % 7 < 3 { 1. } else { -1. })
            .collect();
        for chunk in 0..16 {
            let a = chunk * 1024;
            m.process(&mut l[a..a + 1024], &mut r[a..a + 1024]);
        }
        assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    }

    #[test]
    fn dry_is_the_input_delayed_by_the_latency() {
        let mut m = Machine::new(48000.);
        m.set(Tape {
            mix: 0.,
            ..Tape::default()
        });
        let src = sine(4096, 300., 0.5, 48000.);
        let (mut l, mut r) = (src.clone(), src.clone());
        for chunk in 0..4 {
            let a = chunk * 1024;
            m.process(&mut l[a..a + 1024], &mut r[a..a + 1024]);
        }
        // The glide of the mix from its start (1) to 0 takes a few blocks, so look at the end.
        let d = LATENCY as usize;
        for i in 3500..4000 {
            assert!((l[i] - src[i - d]).abs() < 1e-3, "{i}");
        }
    }

    #[test]
    fn the_wet_path_has_the_latency() {
        // A transparent tape: the wet sine lines up with the delayed input.
        let mut m = Machine::new(48000.);
        m.set(Tape {
            drive: 0.05,
            sat: 0.0,
            bias: 0.5,
            speed: 50.,
            spacing: 0.1,
            gap: 1.,
            ..Tape::default()
        });
        let src = sine(16384, 300., 0.05, 48000.);
        let (mut l, mut r) = (src.clone(), src.clone());
        for chunk in 0..16 {
            let a = chunk * 1024;
            m.process(&mut l[a..a + 1024], &mut r[a..a + 1024]);
        }
        let d = LATENCY as usize;
        let mut best = (f64::MAX, 0);
        for shift in 0..(d + 40) {
            let e: f64 = (12000..16000)
                .map(|i| f64::from(l[i] - src[i - shift]).powi(2))
                .sum();
            if e < best.0 {
                best = (e, shift);
            }
        }
        assert!(
            (best.1 as i64 - d as i64).abs() <= 2,
            "wet is delayed by {} not {d}",
            best.1
        );
    }

    #[test]
    fn loss_darkens() {
        let rate = 48000.;
        let high = sine(16384, 12000., 0.3, rate);
        let mut open = Machine::new(rate);
        open.set(Tape {
            speed: 50.,
            spacing: 0.1,
            ..Tape::default()
        });
        let mut dull = Machine::new(rate);
        dull.set(Tape {
            speed: 3.,
            spacing: 20.,
            ..Tape::default()
        });
        let (mut a, mut b) = (high.clone(), high.clone());
        let (mut a2, mut b2) = (high.clone(), high.clone());
        for chunk in 0..16 {
            let s = chunk * 1024;
            open.process(&mut a[s..s + 1024], &mut a2[s..s + 1024]);
            dull.process(&mut b[s..s + 1024], &mut b2[s..s + 1024]);
        }
        assert!(
            rms(&b[8192..]) < rms(&a[8192..]) * 0.5,
            "{} {}",
            rms(&b[8192..]),
            rms(&a[8192..])
        );
    }
}
