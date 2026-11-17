//! The Room reverb's late field: Freeverb3's `progenitor2`, after the Lexicon 224's plan: modulated input diffusion,
//! two cross-coupled loops of damped allpasses and delays, many output taps, and a wandering comb on each side.

use super::noise::PinkNoise;
use super::prims::{
    AllPass, AllPass2, AllPass3, AllPassM, Base, Biquad, Comb, DcCut, Delay, Iir1, Lfo,
    prime_scaled, scaled,
};

/// The sample rate the design lengths are for.
const DEFAULT_FS: f64 = 34_125.0;
const NUM_IALLPASS: usize = 10;
const NUM_CALLPASS: usize = 4;
const ALLPASS_L: [f64; 8] = [239., 392., 1944., 612., 1212., 121., 816., 1264.];
const ALLPASS_R: [f64; 8] = [205., 329., 2032., 368., 1452., 5., 688., 1340.];
const DELAY_L: [f64; 6] = [1., 2., 1055., 344., 1572., 0.];
const DELAY_R: [f64; 6] = [1., 625., 835., 500., 16., 1460.];
const EXCURSION: f64 = 32.0;
const IDX_OUT: [f64; 11] = [276., 468., 625., 312., 8., 24., 36., 40., 1., 192., 1572.];
const IALLPASS_L: [f64; NUM_IALLPASS] = [617., 535., 434., 347., 218., 162., 144., 122., 109., 74.];
const IALLPASS_R: [f64; NUM_IALLPASS] = [603., 547., 416., 364., 236., 162., 140., 131., 111., 79.];
const CALLPASS_L: [f64; NUM_CALLPASS] = [430., 341., 264., 174.];
const CALLPASS_R: [f64; NUM_CALLPASS] = [447., 324., 247., 191.];
const IDX_OUT2: [f64; 20] = [
    110., 790., 2., 189., 121., 10., 480., 359., 103., 30., 26., 10., 780., 109., 1200., 1310.,
    780., 10., 310., 800.,
];
/// The output lowpass never goes above fs over this.
const OLPF_LIMIT: f64 = 2.5;
const MAX_PRE_DELAY_MS: f64 = 100.0;
const MAX_FS: f64 = 192_000.0;

/// A tap position scaled by the size factor, at least 1 (`f_`).
fn f_(def: f64, factor: f64) -> usize {
    scaled(def, factor)
}

#[derive(Clone)]
pub struct Progenitor2 {
    pub base: Base,
    rt60: f64,
    decay0: f64,
    decay1: f64,
    decay2: f64,
    decay3: f64,
    decay_f: f64,
    dc_cut_fq: f64,
    input_damp: f64,
    damp: f64,
    damp2: f64,
    output_damp: f64,
    output_damp_bw: f64,
    bass_bw: f64,
    bass_boost: f32,
    loop_decay: f32,
    diff: [f64; 4],
    spin: f64,
    spin_limit: f64,
    wander: f32,
    spin2: f64,
    spin2_wander: f64,
    spin_limit2: f64,
    wander2: f32,
    idiff1: f64,
    odiff1: f64,
    mod_noise1: f32,
    mod_noise2: f32,
    cross_feed: f32,
    bass_ap_fc: f64,
    bass_ap_bw: f64,

    dc_l: DcCut,
    dc_r: DcCut,
    lpf_in_l: Iir1,
    lpf_in_r: Iir1,
    lpf_damp_l: Iir1,
    lpf_damp_r: Iir1,
    lpf_bass_l: Biquad,
    lpf_bass_r: Biquad,
    out_lpf_l: Biquad,
    out_lpf_r: Biquad,
    bass_ap_l: Biquad,
    bass_ap_r: Biquad,
    d_l16: Delay,
    d_l23: Delay,
    d_l31: Delay,
    d_l37: Delay,
    d_r49: Delay,
    d_rts: Delay,
    d_r40: Delay,
    d_r58: Delay,
    apm_l15: AllPassM,
    apm_l17: AllPassM,
    apm_r19: AllPassM,
    apm_r21: AllPassM,
    ap2_l: AllPass2,
    ap2_r: AllPass2,
    ap3_l: AllPass3,
    ap3_r: AllPass3,
    lfo1: Lfo,
    lfo2: Lfo,
    lfo1_lpf: Iir1,
    lfo2_lpf: Iir1,
    comb_l: Comb,
    comb_r: Comb,
    noise: PinkNoise,
    iall_l: [AllPassM; NUM_IALLPASS],
    iall_r: [AllPassM; NUM_IALLPASS],
    call_l: [AllPass; NUM_CALLPASS],
    call_r: [AllPass; NUM_CALLPASS],
    out_c: [usize; 11],
    out_c2: [usize; 20],
}

impl Progenitor2 {
    /// A model at `fs` with Freeverb3's defaults, built for size factors up to `max_rs`.
    pub fn new(fs: f64, max_rs: f64) -> Self {
        let mut p = Self {
            base: Base::new(),
            rt60: 2.0,
            decay0: 0.237,
            decay1: 0.938,
            decay2: 0.844,
            decay3: 0.906,
            decay_f: 1.0,
            dc_cut_fq: 5.0,
            input_damp: 20_000.0,
            damp: 9000.0,
            damp2: 500.0,
            output_damp: 10_000.0,
            output_damp_bw: 2.0,
            bass_bw: 2.0,
            bass_boost: 0.1,
            loop_decay: 1.0,
            diff: [0.375, 0.312, 0.406, 0.25],
            spin: 0.5,
            spin_limit: 20.0,
            wander: 0.5,
            spin2: 2.4,
            spin2_wander: 22.0,
            spin_limit2: 12.0,
            wander2: 0.3,
            idiff1: 0.78,
            odiff1: 0.78,
            mod_noise1: 0.09,
            mod_noise2: 0.06,
            cross_feed: 0.4,
            bass_ap_fc: 150.0,
            bass_ap_bw: 4.0,
            dc_l: DcCut::default(),
            dc_r: DcCut::default(),
            lpf_in_l: Iir1::default(),
            lpf_in_r: Iir1::default(),
            lpf_damp_l: Iir1::default(),
            lpf_damp_r: Iir1::default(),
            lpf_bass_l: Biquad::default(),
            lpf_bass_r: Biquad::default(),
            out_lpf_l: Biquad::default(),
            out_lpf_r: Biquad::default(),
            bass_ap_l: Biquad::default(),
            bass_ap_r: Biquad::default(),
            d_l16: Delay::default(),
            d_l23: Delay::default(),
            d_l31: Delay::default(),
            d_l37: Delay::default(),
            d_r49: Delay::default(),
            d_rts: Delay::default(),
            d_r40: Delay::default(),
            d_r58: Delay::default(),
            apm_l15: AllPassM::default(),
            apm_l17: AllPassM::default(),
            apm_r19: AllPassM::default(),
            apm_r21: AllPassM::default(),
            ap2_l: AllPass2::default(),
            ap2_r: AllPass2::default(),
            ap3_l: AllPass3::default(),
            ap3_r: AllPass3::default(),
            lfo1: Lfo::default(),
            lfo2: Lfo::default(),
            lfo1_lpf: Iir1::default(),
            lfo2_lpf: Iir1::default(),
            comb_l: Comb::new(),
            comb_r: Comb::new(),
            noise: PinkNoise::default(),
            iall_l: Default::default(),
            iall_r: Default::default(),
            call_l: Default::default(),
            call_r: Default::default(),
            out_c: [1; 11],
            out_c2: [1; 20],
        };
        p.base.fs = fs;
        p.base.reserve_pre_delay(MAX_PRE_DELAY_MS, MAX_FS);
        // Build the buffers for the largest size and the longest wander comb first, then settle.
        p.base.rs_factor = max_rs.max(1e-3);
        p.spin2_wander = 40.0;
        p.set_fs_factors();
        p.spin2_wander = 22.0;
        p.base.rs_factor = 1.0;
        p.set_fs_factors();
        p.mute();
        p
    }

    pub fn mute(&mut self) {
        self.base.mute();
        self.dc_l.mute();
        self.dc_r.mute();
        for f in [
            &mut self.lpf_in_l,
            &mut self.lpf_in_r,
            &mut self.lpf_damp_l,
            &mut self.lpf_damp_r,
            &mut self.lfo1_lpf,
            &mut self.lfo2_lpf,
        ] {
            f.mute();
        }
        for f in [
            &mut self.lpf_bass_l,
            &mut self.lpf_bass_r,
            &mut self.out_lpf_l,
            &mut self.out_lpf_r,
            &mut self.bass_ap_l,
            &mut self.bass_ap_r,
        ] {
            f.mute();
        }
        for d in [
            &mut self.d_l16,
            &mut self.d_l23,
            &mut self.d_l31,
            &mut self.d_l37,
            &mut self.d_r49,
            &mut self.d_rts,
            &mut self.d_r40,
            &mut self.d_r58,
        ] {
            d.mute();
        }
        for a in [
            &mut self.apm_l15,
            &mut self.apm_l17,
            &mut self.apm_r19,
            &mut self.apm_r21,
        ] {
            a.mute();
        }
        self.ap2_l.mute();
        self.ap2_r.mute();
        self.ap3_l.mute();
        self.ap3_r.mute();
        self.lfo1.mute();
        self.lfo2.mute();
        self.comb_l.mute();
        self.comb_r.mute();
        self.noise.mute();
        for a in self.iall_l.iter_mut().chain(self.iall_r.iter_mut()) {
            a.mute();
        }
        for a in self.call_l.iter_mut().chain(self.call_r.iter_mut()) {
            a.mute();
        }
    }

    pub fn set_sample_rate(&mut self, fs: f64) {
        if fs > 0.0 {
            self.base.fs = fs;
            self.set_fs_factors();
            self.mute();
        }
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

    fn lim_fs2(&self, fq: f64) -> f64 {
        self.base.lim_fs2(fq)
    }

    fn set_fs_factors(&mut self) {
        let predelay = self.base.pre_delay();
        self.base.set_pre_delay(predelay);
        let fs = self.base.fs;
        let total = self.base.total_factor_fs() / DEFAULT_FS;
        let excur = fs / DEFAULT_FS;
        let p = |def: f64| prime_scaled(def, total);
        let pe = |def: f64| prime_scaled(def, excur);

        self.apm_l15.set_size(p(ALLPASS_L[0]), pe(EXCURSION));
        self.apm_l17.set_size(p(ALLPASS_L[1]), pe(EXCURSION));
        self.apm_r19.set_size(p(ALLPASS_R[0]), pe(EXCURSION));
        self.apm_r21.set_size(p(ALLPASS_R[1]), pe(EXCURSION));
        self.ap2_l.set_size(p(ALLPASS_L[2]), p(ALLPASS_L[3]));
        self.ap2_r.set_size(p(ALLPASS_R[2]), p(ALLPASS_R[3]));
        self.ap3_l.set_size(
            p(ALLPASS_L[4]),
            p(ALLPASS_L[5]),
            p(ALLPASS_L[6]),
            p(ALLPASS_L[7]),
        );
        self.ap3_r.set_size(
            p(ALLPASS_R[4]),
            p(ALLPASS_R[5]),
            p(ALLPASS_R[6]),
            p(ALLPASS_R[7]),
        );
        self.d_l16.set_size(p(DELAY_L[1]));
        self.d_l23.set_size(p(DELAY_L[2]));
        self.d_l31.set_size(p(DELAY_L[3]));
        self.d_l37.set_size(p(DELAY_L[4]));
        self.d_rts.set_size(p(1.0));
        self.d_r40.set_size(p(DELAY_R[5]));
        self.d_r49.set_size(p(DELAY_R[3]));
        self.d_r58.set_size(p(DELAY_R[4]));
        for (c, def) in self.out_c.iter_mut().zip(IDX_OUT) {
            *c = f_(def, total);
        }

        // progenitor2's own
        let excur_in = pe((EXCURSION as i64 / 3) as f64);
        for i in 0..NUM_IALLPASS {
            self.iall_l[i].set_size(p(IALLPASS_L[i]), excur_in);
            self.iall_r[i].set_size(p(IALLPASS_R[i]), excur_in);
        }
        for (c, def) in self.out_c2.iter_mut().zip(IDX_OUT2) {
            *c = f_(def, total);
        }
        for i in 0..NUM_CALLPASS {
            self.call_l[i].set_size(p(CALLPASS_L[i]));
            self.call_r[i].set_size(p(CALLPASS_R[i]));
        }

        self.reset_decay();
        self.set_dc_cut(self.dc_cut_fq);
        self.apply_diffusions();
        self.set_input_damp(self.input_damp);
        self.set_damp(self.damp);
        self.set_output_damp(self.output_damp);
        self.set_spin(self.spin);
        self.set_spin_limit(self.spin_limit);
        self.set_spin2_wander(self.spin2_wander);
        self.set_spin2(self.spin2);
        self.set_spin_limit2(self.spin_limit2);
        self.set_damp2(self.damp2);
        self.apply_bass_ap();
    }

    fn apply_diffusions(&mut self) {
        let [d1, d2, d3, d4] = self.diff.map(|v| v as f32);
        self.apm_l15.set_feedback(d1);
        self.apm_r19.set_feedback(d1);
        self.apm_l17.set_feedback(d2);
        self.apm_r21.set_feedback(d2);
        for a in [&mut self.ap2_l, &mut self.ap2_r] {
            a.feedback2 = d3;
            a.feedback1 = d4;
        }
        for a in [&mut self.ap3_l, &mut self.ap3_r] {
            a.feedback3 = d3;
            a.feedback1 = d4;
            a.feedback2 = d4;
        }
        let i = (-self.idiff1) as f32;
        for a in self.iall_l.iter_mut().chain(self.iall_r.iter_mut()) {
            a.set_feedback(i);
        }
        let o = self.odiff1 as f32;
        for a in self.call_l.iter_mut().chain(self.call_r.iter_mut()) {
            a.feedback = o;
        }
    }

    fn reset_decay(&mut self) {
        let rs = self.base.rs_factor;
        let rt60_l = self.rt60 / rs;
        let rt60_s = self.rt60 * self.decay_f / rs;
        let law = |decay: f64, rt: f64| 10f64.powf(decay.log10() / rt) as f32;
        self.loop_decay = law(self.decay0, rt60_l);
        let d1 = law(self.decay1, rt60_s);
        for a in [&mut self.ap2_l, &mut self.ap2_r] {
            a.decay1 = d1;
        }
        for a in [&mut self.ap3_l, &mut self.ap3_r] {
            a.decay1 = d1;
            a.decay2 = d1;
        }
        let d2 = law(self.decay2, rt60_s);
        self.apm_l15.decay = d2;
        self.apm_r19.decay = d2;
        for a in [&mut self.ap2_l, &mut self.ap2_r] {
            a.decay2 = d2;
        }
        for a in [&mut self.ap3_l, &mut self.ap3_r] {
            a.decay3 = d2;
        }
        let d3 = law(self.decay3, rt60_s);
        self.apm_l17.decay = d3;
        self.apm_r21.decay = d3;
    }

    /// The decay time in seconds.
    pub fn set_rt60(&mut self, value: f64) {
        self.rt60 = value;
        self.reset_decay();
    }

    pub fn set_bass_boost(&mut self, v: f32) {
        self.bass_boost = v;
    }

    fn set_dc_cut(&mut self, v: f64) {
        self.dc_cut_fq = self.lim_fs2(v);
        self.dc_l.set_cut_freq(self.dc_cut_fq, self.base.fs);
        self.dc_r.set_cut_freq(self.dc_cut_fq, self.base.fs);
    }

    pub fn set_idiffusion1(&mut self, v: f64) {
        self.idiff1 = v;
        self.apply_diffusions();
    }

    pub fn set_odiffusion1(&mut self, v: f64) {
        self.odiff1 = v;
        self.apply_diffusions();
    }

    fn set_input_damp(&mut self, v: f64) {
        self.input_damp = self.lim_fs2(v);
        self.lpf_in_l.set_lowpass(self.input_damp, self.base.fs);
        self.lpf_in_r.set_lowpass(self.input_damp, self.base.fs);
    }

    /// The loop's lowpass.
    pub fn set_damp(&mut self, v: f64) {
        self.damp = self.lim_fs2(v);
        self.lpf_damp_l.set_lowpass(self.damp, self.base.fs);
        self.lpf_damp_r.set_lowpass(self.damp, self.base.fs);
    }

    /// The bass-boost lowpass's corner.
    pub fn set_damp2(&mut self, v: f64) {
        self.damp2 = self.lim_fs2(v);
        let bw = if self.bass_bw <= 0.0 {
            1.0
        } else {
            self.bass_bw
        };
        self.lpf_bass_l.set_lowpass_bw(self.damp2, bw, self.base.fs);
        self.lpf_bass_r.set_lowpass_bw(self.damp2, bw, self.base.fs);
    }

    /// The output lowpass.
    pub fn set_output_damp(&mut self, v: f64) {
        let v = v.min(self.base.fs / OLPF_LIMIT);
        self.output_damp = self.lim_fs2(v);
        self.out_lpf_l
            .set_lowpass_bw(self.output_damp, self.output_damp_bw, self.base.fs);
        self.out_lpf_r
            .set_lowpass_bw(self.output_damp, self.output_damp_bw, self.base.fs);
    }

    fn apply_bass_ap(&mut self) {
        self.bass_ap_l
            .set_allpass_bw(self.bass_ap_fc, self.bass_ap_bw, self.base.fs);
        self.bass_ap_r
            .set_allpass_bw(self.bass_ap_fc, self.bass_ap_bw, self.base.fs);
    }

    pub fn set_spin(&mut self, v: f64) {
        self.spin = self.lim_fs2(v);
        self.lfo1.set_freq(self.spin / self.base.fs);
    }

    fn set_spin_limit(&mut self, v: f64) {
        self.spin_limit = self.lim_fs2(v);
        self.lfo1_lpf.set_lowpass(self.spin_limit, self.base.fs);
    }

    pub fn set_wander(&mut self, v: f32) {
        self.wander = v.clamp(0.0, 1.0);
    }

    pub fn set_spin2(&mut self, v: f64) {
        self.spin2 = self.lim_fs2(v);
        self.lfo2.set_freq(self.spin2 / self.base.fs);
    }

    fn set_spin2_wander(&mut self, ms: f64) {
        let ms = ms.max(0.0);
        self.spin2_wander = ms;
        let n = prime_scaled(ms, self.base.fs * 0.001);
        self.comb_l.set_size(n);
        self.comb_r.set_size(n);
    }

    fn set_spin_limit2(&mut self, v: f64) {
        self.spin_limit2 = self.lim_fs2(v);
        self.lfo2_lpf.set_lowpass(self.spin_limit2, self.base.fs);
    }

    pub fn set_wander2(&mut self, v: f32) {
        self.wander2 = v.clamp(0.0, 1.0);
    }

    /// One stereo sample in, the reverb's tail out (no dry).
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let mut out_l = self.dc_l.process(in_l);
        let mut out_r = self.dc_r.process(in_r);

        let mut mnoise = self.noise.process();
        let lfo = (self.lfo1.process() + self.mod_noise1 * mnoise) * self.wander;
        let lfo = self.lfo1_lpf.process(lfo);
        mnoise *= self.mod_noise2;

        // input diffusion
        let mut sign = -1.0f32;
        for i in 0..NUM_IALLPASS {
            out_l = self.iall_l[i].process_fmod(out_l, lfo * sign, mnoise);
            out_r = self.iall_r[i].process_fmod(out_r, lfo, mnoise * sign);
            sign = -sign;
        }

        let mut cross_l = out_l;
        let mut cross_r = out_r;
        for i in 0..NUM_CALLPASS {
            cross_l = self.call_l[i].process(cross_l);
            cross_r = self.call_r[i].process(cross_r);
        }
        out_l = self.lpf_in_l.process(out_l + self.cross_feed * cross_r);
        out_r = self.lpf_in_r.process(out_r + self.cross_feed * cross_l);

        // the stereo cross loop
        let cross_r = self.d_r58.last();
        let cross_l = self.d_l37.last();
        let boost_l = self.lpf_bass_l.process(self.bass_ap_l.process(cross_r));
        let boost_r = self.lpf_bass_r.process(self.bass_ap_r.process(cross_l));
        out_l += self.loop_decay * (cross_r + self.bass_boost * boost_l);
        out_r += self.loop_decay * (cross_l + self.bass_boost * boost_r);

        // damping and allpass diffusion
        let damped_l = self.lpf_damp_l.process(out_l);
        let a = self.apm_l15.process_dc_fmod(damped_l, lfo, mnoise);
        let a = self.d_l16.process(a);
        out_l = self.apm_l17.process_dc_fmod(a, -lfo, -mnoise);
        let damped_r = self.lpf_damp_r.process(out_r);
        let b = self.apm_r19.process_dc_fmod(damped_r, -lfo, -mnoise);
        let b = self.d_rts.process(b);
        out_r = self.apm_r21.process_dc_fmod(b, lfo, mnoise);

        let a = self.d_l23.process(out_l);
        let a = self.ap2_l.process(a);
        let a = self.d_l31.process(a);
        let a = self.ap3_l.process_mod(a, lfo);
        self.d_l37.process(a);
        let b = self.d_r40.process(out_r);
        let b = self.ap2_r.process(b);
        let b = self.d_r49.process(b);
        let b = self.ap3_r.process_mod(b, -lfo);
        self.d_r58.process(b);

        let (c, c2) = (&self.out_c, &self.out_c2);
        let d_out = self.d_l23.get_z(c[8]) * 0.469
            + (self.d_l31.get_z(c[7]) - self.d_r49.get_z(c[9]) + self.d_l31.get_z(c[0])
                - self.d_r40.get_z(c2[0])
                - self.d_r49.get_z(c[1]))
                * 0.219
            + (self.d_l37.get_z(c[10]) + self.ap2_l.get_z1(c2[4]) + self.ap2_l.get_z2(c2[6])
                - self.ap2_r.get_z2(c2[8])
                + self.ap3_l.get_z1(c2[10])
                + self.ap3_l.get_z2(c2[12])
                + self.ap3_l.get_z3(c2[14])
                - self.ap3_r.get_z2(c2[18]))
                * 0.064
            + self.d_l37.get_z(c2[16]) * 0.045;
        let b_out = self.d_r40.get_z(c[2]) * 0.469
            + (self.d_r49.get_z(c[1]) - self.d_l31.get_z(c[3]) + self.d_r49.get_z(c[5])
                - self.d_l37.get_z(c[6])
                - self.d_l23.get_z(c2[1])
                - self.d_l31.get_z(c2[3]))
                * 0.219
            + (self.d_r58.get_z(c[4]) + self.ap2_r.get_z1(c2[5]) + self.ap2_r.get_z2(c2[7])
                - self.ap2_l.get_z2(c2[9])
                + self.ap3_r.get_z1(c2[11])
                + self.ap3_r.get_z2(c2[13])
                + self.ap3_r.get_z3(c2[15])
                - self.ap3_l.get_z2(c2[19]))
                * 0.064
            + self.d_r58.get_z(c2[17]) * 0.045;

        let lfo = self.lfo2_lpf.process(self.lfo2.process() * self.wander2);
        self.comb_l.feedback = lfo;
        self.comb_r.feedback = -lfo;
        let out_l = self.comb_l.process_ff(d_out);
        let out_r = self.comb_r.process_ff(b_out);

        let fp_l = self.base.delay_wl.process(self.out_lpf_l.process(out_l));
        let fp_r = self.base.delay_wr.process(self.out_lpf_r.process(out_r));
        (
            fp_l * self.base.wet1 + fp_r * self.base.wet2,
            fp_r * self.base.wet1 + fp_l * self.base.wet2,
        )
    }
}
