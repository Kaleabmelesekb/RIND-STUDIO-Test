//! Early reflections: the taps of a room's first echoes (Freeverb3's `earlyref`, as set up by Dragonfly Early
//! Reflections and by the Hall and Room reverbs).
//!
//! Each side reads a tap table off its own delay line, the two sides leak into each other through a short delay and an
//! allpass, and an allpass, a lowpass and a highpass shape the sum. Dry is muted (the host mixes it), so the output is
//! the reflections alone.

use super::early_taps::{Taps, preset};
use super::prims::{Base, Biquad, Delay, Iir1};

/// A plain delay line read at whole-sample taps.
#[derive(Clone, Default)]
struct DelayLine {
    buf: Vec<f32>,
    base: usize,
}

impl DelayLine {
    fn set_size(&mut self, size: usize) {
        self.buf.clear();
        self.buf.resize(size.max(1), 0.0);
        self.base = 0;
    }

    fn mute(&mut self) {
        self.buf.fill(0.0);
    }

    #[inline]
    fn push(&mut self, input: f32) {
        let n = self.buf.len();
        self.base = if self.base == 0 { n - 1 } else { self.base - 1 };
        self.buf[self.base] = input;
    }

    #[inline]
    fn at(&self, tap: usize) -> f32 {
        let n = self.buf.len();
        let mut i = self.base + tap;
        if i >= n {
            i -= n;
        }
        self.buf[i]
    }
}

/// One side's tap: delay in samples (fractional design value, read at its whole part) and gain.
type Tap = (f32, f32);

#[derive(Clone)]
pub struct EarlyReflections {
    pub base: Base,
    preset: u32,
    left: Vec<Tap>,
    right: Vec<Tap>,
    line_l: DelayLine,
    line_r: DelayLine,
    l_to_r: Delay,
    r_to_l: Delay,
    cross_l: Biquad,
    cross_r: Biquad,
    diffuse_l: Biquad,
    diffuse_r: Biquad,
    lp_l: Iir1,
    lp_r: Iir1,
    hp_l: Iir1,
    hp_r: Iir1,
    lr_delay_ms: f64,
    cross_fq: f64,
    cross_bw: f64,
    diffuse_fq: f64,
    diffuse_bw: f64,
    out_lpf: f64,
    out_hpf: f64,
}

/// The longest size factor the reverbs use for the early reflections (Size 60 m over 10): the buffers are built for it.
pub const MAX_RS_FACTOR: f64 = 6.0;

impl EarlyReflections {
    /// A model at `fs` whose buffers hold up to [`MAX_RS_FACTOR`] times the table's delays, set up as Dragonfly sets
    /// it: dry muted, wet 0 dB, width 0.8.
    pub fn new(fs: f64, preset_id: u32) -> Self {
        let mut e = Self {
            base: Base::new(),
            preset: preset_id,
            left: Vec::new(),
            right: Vec::new(),
            line_l: DelayLine::default(),
            line_r: DelayLine::default(),
            l_to_r: Delay::default(),
            r_to_l: Delay::default(),
            cross_l: Biquad::default(),
            cross_r: Biquad::default(),
            diffuse_l: Biquad::default(),
            diffuse_r: Biquad::default(),
            lp_l: Iir1::default(),
            lp_r: Iir1::default(),
            hp_l: Iir1::default(),
            hp_r: Iir1::default(),
            lr_delay_ms: 0.3,
            cross_fq: 750.0,
            cross_bw: 4.0,
            diffuse_fq: 150.0,
            diffuse_bw: 4.0,
            out_lpf: 20_000.0,
            out_hpf: 4.0,
        };
        e.base.set_dry_ratio(0.0);
        e.base.set_wet_db(0.0);
        e.base.set_width(0.8);
        e.base.fs = fs;
        // Room for the longest preset's taps and delays at the largest factor, so a later preset or size never
        // allocates.
        let (mut taps, mut longest) = (0usize, 0.0f32);
        for t in super::early_taps::PRESETS.iter() {
            taps = taps.max(t.left.len()).max(t.right.len());
            for &(d, _) in t.left.iter().chain(t.right.iter()) {
                longest = longest.max(d);
            }
        }
        e.left.reserve(taps);
        e.right.reserve(taps);
        let line = (f64::from(longest) * fs * MAX_RS_FACTOR) as usize + 16;
        e.line_l.buf.reserve(line);
        e.line_r.buf.reserve(line);
        // Size the buffers for the largest factor, then settle on the first.
        e.base.rs_factor = MAX_RS_FACTOR;
        e.set_fs_factors();
        e.base.rs_factor = 1.0;
        e.set_fs_factors();
        e.set_output_lpf(20_000.0);
        e.set_output_hpf(4.0);
        e.mute();
        e
    }

    pub fn mute(&mut self) {
        self.base.mute();
        self.line_l.mute();
        self.line_r.mute();
        self.l_to_r.mute();
        self.r_to_l.mute();
        self.cross_l.mute();
        self.cross_r.mute();
        self.diffuse_l.mute();
        self.diffuse_r.mute();
        self.lp_l.mute();
        self.lp_r.mute();
        self.hp_l.mute();
        self.hp_r.mute();
    }

    pub fn set_width(&mut self, w: f32) {
        self.base.set_width(w);
    }

    /// Sample rate changed: rebuild what depends on it.
    pub fn set_sample_rate(&mut self, fs: f64) {
        if fs > 0.0 {
            self.base.fs = fs;
            self.set_fs_factors();
            self.set_output_lpf(self.out_lpf);
            self.set_output_hpf(self.out_hpf);
            self.mute();
        }
    }

    /// The size factor (Size in metres over 10 for the reverbs).
    pub fn set_rs_factor(&mut self, factor: f64) {
        if factor <= 0.0 {
            return;
        }
        self.base.rs_factor = factor.min(MAX_RS_FACTOR);
        self.set_fs_factors();
    }

    /// Re-derives everything that depends on the sample rate or the size factor (Freeverb3's `setFsFactors`).
    fn set_fs_factors(&mut self) {
        self.set_lr_delay(0.2);
        self.set_cross_allpass(self.cross_fq, self.cross_bw);
        self.set_diffusion_allpass(self.diffuse_fq, self.diffuse_bw);
        self.load_preset(self.preset);
    }

    pub fn set_lr_delay(&mut self, ms: f64) {
        self.lr_delay_ms = ms;
        let samples = (self.base.fs * ms / 1000.0) as usize;
        self.r_to_l.set_size(samples);
        self.l_to_r.set_size(samples);
    }

    pub fn set_cross_allpass(&mut self, fc: f64, bw: f64) {
        self.cross_fq = fc;
        self.cross_bw = bw;
        self.cross_l.set_allpass_bw(fc, bw, self.base.fs);
        self.cross_r.set_allpass_bw(fc, bw, self.base.fs);
    }

    pub fn set_diffusion_allpass(&mut self, fc: f64, bw: f64) {
        self.diffuse_fq = fc;
        self.diffuse_bw = bw;
        self.diffuse_l.set_allpass_bw(fc, bw, self.base.fs);
        self.diffuse_r.set_allpass_bw(fc, bw, self.base.fs);
    }

    pub fn set_output_lpf(&mut self, hz: f64) {
        self.out_lpf = hz.clamp(0.0, self.base.fs / 2.0);
        self.lp_l.set_lowpass(self.out_lpf, self.base.fs);
        self.lp_r.set_lowpass(self.out_lpf, self.base.fs);
    }

    pub fn set_output_hpf(&mut self, hz: f64) {
        self.out_hpf = hz.clamp(0.0, self.base.fs / 2.0);
        self.hp_l.set_highpass(self.out_hpf, self.base.fs);
        self.hp_r.set_highpass(self.out_hpf, self.base.fs);
    }

    /// Loads Freeverb3 preset `id` (0, 1, 2 or 11 … 22; any other is 0), scaled by the sample rate and size factor.
    pub fn load_preset(&mut self, id: u32) {
        let taps: &Taps = preset(id);
        self.preset = taps.id;
        let k = self.base.total_factor_fs() as f32;
        self.left.clear();
        self.left.extend(taps.left.iter().map(|&(d, g)| (d * k, g)));
        self.right.clear();
        self.right
            .extend(taps.right.iter().map(|&(d, g)| (d * k, g)));
        let longest = |t: &[Tap]| t.iter().fold(0.0f32, |m, &(d, _)| m.max(d));
        self.line_l.set_size(longest(&self.left) as usize + 10);
        self.line_r.set_size(longest(&self.right) as usize + 10);
        self.mute();
    }

    pub fn preset(&self) -> u32 {
        self.preset
    }

    /// One stereo sample of reflections (the dry signal is muted).
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        self.line_l.push(in_l);
        self.line_r.push(in_r);
        let (mut wet_l, mut wet_r) = (0.0f32, 0.0f32);
        for &(d, g) in &self.left {
            wet_l += g * self.line_l.at(d as usize);
        }
        for &(d, g) in &self.right {
            wet_r += g * self.line_r.at(d as usize);
        }
        let wet_l = self.base.delay_wl.process(wet_l);
        let wet_r = self.base.delay_wr.process(wet_r);
        let (w1, w2) = (self.base.wet1, self.base.wet2);
        let to_l = self.cross_l.process(self.r_to_l.process(in_r + wet_r));
        let to_r = self.cross_r.process(self.l_to_r.process(in_l + wet_l));
        let out_l = self.diffuse_l.process(w1 * wet_l + w2 * to_l);
        let out_r = self.diffuse_r.process(w1 * wet_r + w2 * to_r);
        (
            self.lp_l.process(self.hp_l.process(out_l)),
            self.lp_r.process(self.hp_r.process(out_r)),
        )
    }
}
