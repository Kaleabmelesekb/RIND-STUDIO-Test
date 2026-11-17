//! The Hall reverb's late field: Freeverb3's `zrev2`, a feedback delay network after Fons Adriaensen's zita-rev1 with
//! ten input-diffusing allpasses a side, eight modulated delay lines with frequency-dependent decay, a spinning
//! feed-forward comb and the output filters.

use super::prims::{
    AllPassM, Base, Biquad, Comb, DcCut, DelayM, Iir1, Lfo, prime_scaled, ratio_to_db, scaled,
};

const NUM_DELAYS: usize = 8;
const NUM_IALLPASS: usize = 10;
/// The sample rate the input allpass lengths are designed for.
const ALLPASS_FS: f64 = 34_125.0;
const IALLPASS_L: [f64; NUM_IALLPASS] = [617., 535., 434., 347., 218., 162., 144., 122., 109., 74.];
const IALLPASS_R: [f64; NUM_IALLPASS] = [603., 547., 416., 364., 236., 162., 140., 131., 111., 79.];
const ALLPASS_EXCURSION: f64 = 32.0;
/// Seconds: the 8 lines' lengths with and without their diffusing allpass, and the modulation room.
const DELAY_REAL: [f64; NUM_DELAYS] = [
    0.153129, 0.210389, 0.127837, 0.256891, 0.174713, 0.192303, 0.125, 0.219991,
];
const DELAY_DIFF: [f64; NUM_DELAYS] = [
    0.020346, 0.024421, 0.031604, 0.027333, 0.022904, 0.029291, 0.013458, 0.019123,
];
const DELAY_EXCURSION: f64 = 0.001;
/// The longest predelay and the highest sample rate the buffers are built for.
const MAX_PRE_DELAY_MS: f64 = 100.0;
const MAX_FS: f64 = 192_000.0;

#[derive(Clone)]
pub struct Zrev2 {
    pub base: Base,
    rt60: f64,
    ap_feedback: f64,
    loop_damp: f64,
    output_lpf: f64,
    output_hpf: f64,
    dc_cut_fq: f64,
    lfo1_freq: f64,
    lfo2_freq: f64,
    lfo_factor: f32,
    rt60_f_low: f64,
    rt60_f_high: f64,
    xo_low: f64,
    xo_high: f64,
    idiff1: f64,
    wander_ms: f64,
    spin_fq: f64,
    spin_factor: f32,
    diff1: [AllPassM; NUM_DELAYS],
    delay: [DelayM; NUM_DELAYS],
    lsf: [Biquad; NUM_DELAYS],
    hsf: [Biquad; NUM_DELAYS],
    iall_l: [AllPassM; NUM_IALLPASS],
    iall_r: [AllPassM; NUM_IALLPASS],
    dc_l: DcCut,
    dc_r: DcCut,
    out_lpf: [Iir1; 2],
    out_hpf: [Iir1; 2],
    lfo1: Lfo,
    lfo2: Lfo,
    lfo1_lpf: Iir1,
    lfo2_lpf: Iir1,
    spin_lfo: Lfo,
    spin_lpf: Iir1,
    spin_comb_l: Comb,
    spin_comb_r: Comb,
}

impl Zrev2 {
    /// A model at `fs` with Freeverb3's defaults whose buffers hold up to `max_rs` times the design sizes (so that
    /// the size factor can move within it without allocating).
    pub fn new(fs: f64, max_rs: f64) -> Self {
        let mut z = Self {
            base: Base::new(),
            rt60: 2.0,
            ap_feedback: 0.6,
            loop_damp: 3600.0,
            output_lpf: 10_000.0,
            output_hpf: 4.0,
            dc_cut_fq: 2.5,
            lfo1_freq: 0.9,
            lfo2_freq: 1.3,
            lfo_factor: 0.31,
            rt60_f_low: 1.3,
            rt60_f_high: 0.3,
            xo_low: 500.0,
            xo_high: 3600.0,
            idiff1: 0.78,
            wander_ms: 22.0,
            spin_fq: 2.4,
            spin_factor: 0.3,
            diff1: Default::default(),
            delay: Default::default(),
            lsf: Default::default(),
            hsf: Default::default(),
            iall_l: Default::default(),
            iall_r: Default::default(),
            dc_l: DcCut::default(),
            dc_r: DcCut::default(),
            out_lpf: [Iir1::default(); 2],
            out_hpf: [Iir1::default(); 2],
            lfo1: Lfo::default(),
            lfo2: Lfo::default(),
            lfo1_lpf: Iir1::default(),
            lfo2_lpf: Iir1::default(),
            spin_lfo: Lfo::default(),
            spin_lpf: Iir1::default(),
            spin_comb_l: Comb::new(),
            spin_comb_r: Comb::new(),
        };
        z.base.fs = fs;
        z.base.reserve_pre_delay(MAX_PRE_DELAY_MS, MAX_FS);
        z.base.rs_factor = max_rs.max(1e-3);
        // The comb's size follows the wander, which has its own ceiling (40 ms): size it first.
        z.wander_ms = 40.0;
        z.set_fs_factors();
        z.wander_ms = 22.0;
        z.base.rs_factor = 1.0;
        z.set_fs_factors();
        z.mute();
        z
    }

    pub fn mute(&mut self) {
        self.base.mute();
        for i in 0..NUM_DELAYS {
            self.diff1[i].mute();
            self.delay[i].mute();
            self.lsf[i].mute();
            self.hsf[i].mute();
        }
        for i in 0..NUM_IALLPASS {
            self.iall_l[i].mute();
            self.iall_r[i].mute();
        }
        self.lfo1.mute();
        self.lfo2.mute();
        self.lfo1_lpf.mute();
        self.lfo2_lpf.mute();
        self.dc_l.mute();
        self.dc_r.mute();
        for f in self.out_lpf.iter_mut().chain(self.out_hpf.iter_mut()) {
            f.mute();
        }
        self.spin_lfo.mute();
        self.spin_lpf.mute();
        self.spin_comb_l.mute();
        self.spin_comb_r.mute();
    }

    /// Sample rate changed: rebuild everything that depends on it.
    pub fn set_sample_rate(&mut self, fs: f64) {
        self.base.fs = fs;
        let w = self.base.pre_delay();
        self.base.set_pre_delay(w);
        self.set_fs_factors();
        self.mute();
    }

    pub fn set_rs_factor(&mut self, factor: f64) {
        if factor > 0.0 {
            self.base.rs_factor = factor;
            self.set_fs_factors();
        }
    }

    pub fn set_pre_delay(&mut self, ms: f64) {
        self.base.set_pre_delay(ms);
    }

    pub fn set_width(&mut self, w: f32) {
        self.base.set_width(w);
    }

    fn set_fs_factors(&mut self) {
        let predelay = self.base.pre_delay();
        self.base.set_pre_delay(predelay);
        let fs = self.base.fs;
        let total = self.base.total_factor_fs();
        for i in 0..NUM_DELAYS {
            self.delay[i].set_size(
                prime_scaled(DELAY_REAL[i] - DELAY_DIFF[i], total),
                scaled(DELAY_EXCURSION, fs),
            );
            self.diff1[i].set_size(
                prime_scaled(DELAY_DIFF[i], total),
                scaled(DELAY_EXCURSION, fs),
            );
        }
        let total_factor = total / ALLPASS_FS;
        let excur_factor = fs / ALLPASS_FS;
        let excur = prime_scaled((ALLPASS_EXCURSION as i64 / 3) as f64, excur_factor);
        for i in 0..NUM_IALLPASS {
            self.iall_l[i].set_size(prime_scaled(IALLPASS_L[i], total_factor), excur);
            self.iall_r[i].set_size(prime_scaled(IALLPASS_R[i], total_factor), excur);
        }
        self.set_rt60(self.rt60);
        self.set_ap_feedback(self.ap_feedback);
        self.set_loop_damp_raw(self.loop_damp);
        self.set_output_lpf(self.output_lpf);
        self.set_output_hpf(self.output_hpf);
        self.set_dc_cut(self.dc_cut_fq);
        self.set_lfo1_freq(self.lfo1_freq);
        self.set_lfo2_freq(self.lfo2_freq);
        self.set_idiffusion1(self.idiff1);
        self.set_wander(self.wander_ms);
        self.set_spin(self.spin_fq);
    }

    /// Decay time of the whole field in seconds (at the mid band).
    pub fn set_rt60(&mut self, value: f64) {
        self.rt60 = value;
        let mut gain = (1.0 / NUM_DELAYS as f64).sqrt();
        let mut back = self.rt60 * self.base.fs;
        if self.rt60 <= 0.0 {
            gain = 0.0;
            back = 1.0;
        }
        let fs = self.base.fs;
        for i in 0..NUM_DELAYS {
            let len = (self.delay[i].size() + self.diff1[i].size()) as f64;
            self.delay[i].feedback = (gain * 10f64.powf(-3.0 * len / back)) as f32;
            let low = 10f64.powf(-3.0 * len / back / self.rt60_f_low * (1.0 - self.rt60_f_low));
            let high = 10f64.powf(-3.0 * len / back / self.rt60_f_high * (1.0 - self.rt60_f_high));
            self.lsf[i].set_low_shelf(self.xo_low, ratio_to_db(low), 1.0, fs);
            self.hsf[i].set_high_shelf(self.xo_high, ratio_to_db(high), 1.0, fs);
        }
    }

    pub fn set_rt60_factor_low(&mut self, v: f64) {
        self.rt60_f_low = v;
        self.set_rt60(self.rt60);
    }

    pub fn set_rt60_factor_high(&mut self, v: f64) {
        self.rt60_f_high = v;
        self.set_rt60(self.rt60);
    }

    pub fn set_xover_low(&mut self, fc: f64) {
        self.xo_low = self.base.lim_fs2(fc);
        self.set_rt60(self.rt60);
    }

    pub fn set_xover_high(&mut self, fc: f64) {
        self.set_loop_damp_raw(fc);
        self.xo_high = self.base.lim_fs2(fc);
        self.set_rt60(self.rt60);
    }

    fn set_loop_damp_raw(&mut self, v: f64) {
        self.loop_damp = self.base.lim_fs2(v);
    }

    pub fn set_ap_feedback(&mut self, v: f64) {
        self.ap_feedback = v;
        let mut rev = 1.0;
        for d in &mut self.diff1 {
            d.set_feedback((rev * v) as f32);
            rev = -rev;
        }
    }

    pub fn set_idiffusion1(&mut self, v: f64) {
        self.idiff1 = v;
        for a in self.iall_l.iter_mut().chain(self.iall_r.iter_mut()) {
            a.set_feedback((-v) as f32);
        }
    }

    /// The diffusion control: both allpass feedbacks follow it.
    pub fn set_diffusion(&mut self, v: f64) {
        self.set_idiffusion1(v);
        self.set_ap_feedback(v);
    }

    pub fn set_output_lpf(&mut self, v: f64) {
        self.output_lpf = self.base.lim_fs2(v);
        for f in &mut self.out_lpf {
            f.set_lowpass(self.output_lpf, self.base.fs);
        }
    }

    pub fn set_output_hpf(&mut self, v: f64) {
        self.output_hpf = self.base.lim_fs2(v);
        for f in &mut self.out_hpf {
            f.set_highpass(self.output_hpf, self.base.fs);
        }
    }

    fn set_dc_cut(&mut self, v: f64) {
        self.dc_cut_fq = self.base.lim_fs2(v);
        self.dc_l.set_cut_freq(self.dc_cut_fq, self.base.fs);
        self.dc_r.set_cut_freq(self.dc_cut_fq, self.base.fs);
    }

    fn set_lfo1_freq(&mut self, fq: f64) {
        self.lfo1_freq = self.base.lim_fs2(fq);
        self.lfo1.set_freq(self.lfo1_freq / self.base.fs);
        self.lfo1_lpf.set_lowpass(self.lfo1_freq, self.base.fs);
    }

    fn set_lfo2_freq(&mut self, fq: f64) {
        self.lfo2_freq = self.base.lim_fs2(fq);
        self.lfo2.set_freq(self.lfo2_freq / self.base.fs);
        self.lfo2_lpf.set_lowpass(self.lfo2_freq, self.base.fs);
    }

    pub fn set_lfo_factor(&mut self, v: f32) {
        self.lfo_factor = v;
    }

    pub fn set_spin_factor(&mut self, v: f32) {
        self.spin_factor = v;
    }

    pub fn set_wander(&mut self, ms: f64) {
        let ms = ms.max(0.0);
        self.wander_ms = ms;
        let n = prime_scaled(ms, self.base.fs * 0.001);
        self.spin_comb_l.set_size(n);
        self.spin_comb_r.set_size(n);
    }

    pub fn set_spin(&mut self, fq: f64) {
        self.spin_fq = self.base.lim_fs2(fq);
        self.spin_lfo.set_freq(self.spin_fq / self.base.fs);
        self.spin_lpf.set_lowpass(self.spin_fq, self.base.fs);
    }

    /// One line's way into the mixing matrix: its high and low shelves, then the diffusing allpass.
    #[inline]
    fn loop_in(&mut self, i: usize, x: f32, m: f32) -> f32 {
        let v = self.hsf[i].process(x);
        let v = self.lsf[i].process(v);
        self.diff1[i].process(v, m)
    }

    /// One stereo sample in, one stereo sample of the reverb's tail out (no dry).
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let lfo1q = self.lfo1_lpf.process(self.lfo1.process() * self.lfo_factor);
        let lfo2q = self.lfo2_lpf.process(self.lfo2.process() * self.lfo_factor);
        let lfo1p = -lfo1q;
        let lfo2p = -lfo2q;

        let mut out_l = self.dc_l.process(in_l);
        let mut out_r = self.dc_r.process(in_r);

        let mut sign = -1.0f32;
        for i in 0..NUM_IALLPASS {
            out_l = self.iall_l[i].process(out_l, lfo1q * sign);
            out_r = self.iall_r[i].process(out_r, lfo2p * sign);
            sign = -sign;
        }

        let last: [f32; NUM_DELAYS] = std::array::from_fn(|i| self.delay[i].last());
        let mut x0 = self.loop_in(0, last[0] + out_l, lfo1q);
        let mut x1 = self.loop_in(1, last[1] + out_l, lfo1p);
        let mut x2 = self.loop_in(2, last[2] - out_l, lfo1q);
        let mut x3 = self.loop_in(3, last[3] - out_l, lfo1p);
        let mut x4 = self.loop_in(4, last[4] + out_r, lfo2p);
        let mut x5 = self.loop_in(5, last[5] + out_r, lfo2q);
        let mut x6 = self.loop_in(6, last[6] - out_r, lfo2p);
        let mut x7 = self.loop_in(7, last[7] - out_r, lfo2q);

        let mut t = x0 - x1;
        x0 += x1;
        x1 = t;
        t = x2 - x3;
        x2 += x3;
        x3 = t;
        t = x4 - x5;
        x4 += x5;
        x5 = t;
        t = x6 - x7;
        x6 += x7;
        x7 = t;
        t = x0 - x2;
        x0 += x2;
        x2 = t;
        t = x1 - x3;
        x1 += x3;
        x3 = t;
        t = x4 - x6;
        x4 += x6;
        x6 = t;
        t = x5 - x7;
        x5 += x7;
        x7 = t;
        t = x0 - x4;
        x0 += x4;
        x4 = t;
        t = x1 - x5;
        x1 += x5;
        x5 = t;
        t = x2 - x6;
        x2 += x6;
        x6 = t;
        t = x3 - x7;
        x3 += x7;
        x7 = t;

        self.delay[0].process(x0, lfo2q);
        self.delay[1].process(x1, lfo1q);
        self.delay[2].process(x2, lfo2p);
        self.delay[3].process(x3, lfo1p);
        self.delay[4].process(x4, lfo1p);
        self.delay[5].process(x5, lfo2q);
        self.delay[6].process(x6, lfo1p);
        self.delay[7].process(x7, lfo2q);

        let wl = 0.2 * (x0 - x1 + x2 - x3);
        let wr = 0.2 * (x4 + x5 - x6 - x7);

        let spin = self
            .spin_lpf
            .process(self.spin_lfo.process() * self.spin_factor);
        self.spin_comb_l.feedback = spin;
        self.spin_comb_r.feedback = -spin;
        let wl = self.spin_comb_l.process_ff(wl);
        let wr = self.spin_comb_r.process_ff(wr);

        let fp_l = self
            .base
            .delay_wl
            .process(self.out_lpf[0].process(self.out_hpf[0].process(wl)));
        let fp_r = self
            .base
            .delay_wr
            .process(self.out_lpf[1].process(self.out_hpf[1].process(wr)));
        (
            fp_l * self.base.wet1 + fp_r * self.base.wet2,
            fp_r * self.base.wet1 + fp_l * self.base.wet2,
        )
    }
}
