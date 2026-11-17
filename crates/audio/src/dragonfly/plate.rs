//! The Plate reverb: Freeverb3's three plate algorithms as Dragonfly Plate uses them, "Simple" (`nrev`: six lowpass
//! combs and allpasses after John Chowning's/Stanford's NRev), "Nested" (`nrevb`: NRev with twelve more combs and
//! three more allpasses) and "Tank" (`strev`: Dattorro's plate with a modulated, noise-driven tank).

use super::noise::PinkNoise;
use super::prims::{AllPass, AllPassM, Base, Comb, DcCut, Delay, Iir1, Lfo, prime_scaled, scaled};

const NREV_FS: f64 = 25_641.0;
const STEREO_SPREAD: f64 = 13.0;
const NREV_SCALE_WET: f32 = 0.05;
const NREVB_SCALE_WET: f32 = 0.4;
const NUM_COMB: usize = 6;
const NUM_ALLPASS: usize = 9;
const NUM_COMB2: usize = 12;
const NUM_ALLPASS2: usize = 3;
const COMB_CO: [f64; NUM_COMB] = [1433., 1601., 1867., 2053., 2251., 2399.];
const ALLPASS_CO: [f64; NUM_ALLPASS] = [347., 113., 37., 59., 53., 43., 37., 29., 19.];
const COMB_CO2: [f64; NUM_COMB2] = [
    1257., 1333., 1499., 1501., 1535., 1637., 1738., 1904., 1999., 2020., 2120., 2509.,
];
const ALLPASS_CO2: [f64; NUM_ALLPASS2] = [57., 87., 103.];

const STREV_FS: f64 = 29_761.0;
const STREV_ALLP: [f64; 8] = [142., 107., 379., 277., 672., 908., 1800., 2656.];
const STREV_DELAY: [f64; 4] = [4453., 3720., 4217., 3163.];
const STREV_IDX_L: [f64; 7] = [266., 2974., 1913., 1996., 1990., 187., 1066.];
const STREV_IDX_R: [f64; 7] = [353., 3627., 1228., 2673., 2111., 335., 121.];
const STREV_EXCURSION: f64 = 32.0;
const MAX_PRE_DELAY_MS: f64 = 100.0;
const MAX_FS: f64 = 192_000.0;

/// The three plate algorithms, in the Plate's Algorithm control's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Simple,
    Nested,
    Tank,
}

impl Algorithm {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Simple,
            1 => Self::Nested,
            _ => Self::Tank,
        }
    }
}

/// `nrev` and `nrevb` together: the Nested one has the extra combs and allpasses.
#[derive(Clone)]
struct NRev {
    nested: bool,
    rt60: f64,
    feedback: f32,
    ap_feedback: f32,
    damp: f32,
    damp2: f32,
    damp3: f32,
    damp_lpf: f64,
    dc_cut_fq: f64,
    comb_l: [Comb; NUM_COMB],
    comb_r: [Comb; NUM_COMB],
    comb2_l: [Comb; NUM_COMB2],
    comb2_r: [Comb; NUM_COMB2],
    allpass_l: [AllPass; NUM_ALLPASS],
    allpass_r: [AllPass; NUM_ALLPASS],
    allpass2_l: [AllPass; NUM_ALLPASS2],
    allpass2_r: [AllPass; NUM_ALLPASS2],
    in_dc: DcCut,
    l_dc: DcCut,
    r_dc: DcCut,
    damp_lpf_l: Iir1,
    damp_lpf_r: Iir1,
    hpf: f32,
    lpf_l: f32,
    lpf_r: f32,
    last_l: f32,
    last_r: f32,
}

impl NRev {
    fn new(nested: bool) -> Self {
        Self {
            nested,
            rt60: 1.0,
            feedback: if nested { 0.5 } else { 0.7 },
            ap_feedback: 0.2,
            damp: if nested { 0.1 } else { 0.5 },
            damp2: 0.5,
            damp3: 0.5,
            damp_lpf: 20_000.0,
            dc_cut_fq: 8.0,
            comb_l: std::array::from_fn(|_| Comb::new()),
            comb_r: std::array::from_fn(|_| Comb::new()),
            comb2_l: std::array::from_fn(|_| Comb::new()),
            comb2_r: std::array::from_fn(|_| Comb::new()),
            allpass_l: Default::default(),
            allpass_r: Default::default(),
            allpass2_l: Default::default(),
            allpass2_r: Default::default(),
            in_dc: DcCut::default(),
            l_dc: DcCut::default(),
            r_dc: DcCut::default(),
            damp_lpf_l: Iir1::default(),
            damp_lpf_r: Iir1::default(),
            hpf: 0.0,
            lpf_l: 0.0,
            lpf_r: 0.0,
            last_l: 0.0,
            last_r: 0.0,
        }
    }

    fn mute(&mut self) {
        for c in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            c.mute();
        }
        for c in self.comb2_l.iter_mut().chain(self.comb2_r.iter_mut()) {
            c.mute();
        }
        for a in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            a.mute();
        }
        for a in self.allpass2_l.iter_mut().chain(self.allpass2_r.iter_mut()) {
            a.mute();
        }
        self.in_dc.mute();
        self.l_dc.mute();
        self.r_dc.mute();
        self.damp_lpf_l.mute();
        self.damp_lpf_r.mute();
        (self.hpf, self.lpf_l, self.lpf_r) = (0.0, 0.0, 0.0);
        (self.last_l, self.last_r) = (0.0, 0.0);
    }

    fn set_fs_factors(&mut self, base: &Base) {
        let total = base.total_factor_fs() / NREV_FS;
        let spread = scaled(STEREO_SPREAD, total) as f64;
        for i in 0..NUM_COMB {
            self.comb_l[i].set_size(prime_scaled(COMB_CO[i], total));
            self.comb_r[i].set_size(prime_scaled(scaled(COMB_CO[i], total) as f64 + spread, 1.0));
        }
        for i in 0..NUM_ALLPASS {
            self.allpass_l[i].set_size(prime_scaled(ALLPASS_CO[i], total));
            self.allpass_r[i].set_size(prime_scaled(
                scaled(ALLPASS_CO[i], total) as f64 + spread,
                1.0,
            ));
        }
        if self.nested {
            for i in 0..NUM_COMB2 {
                self.comb2_l[i].set_size(prime_scaled(COMB_CO2[i], total));
                self.comb2_r[i].set_size(prime_scaled(
                    scaled(COMB_CO2[i], total) as f64 + spread,
                    1.0,
                ));
            }
            for i in 0..NUM_ALLPASS2 {
                self.allpass2_l[i].set_size(prime_scaled(ALLPASS_CO2[i], total));
                self.allpass2_r[i].set_size(prime_scaled(
                    scaled(ALLPASS_CO2[i], total) as f64 + spread,
                    1.0,
                ));
            }
        }
        self.apply_feedbacks();
        self.apply_damp();
        self.set_rt60(base, self.rt60);
        self.set_dc_cut(base, self.dc_cut_fq);
        self.set_damp_lpf(base, self.damp_lpf);
    }

    fn apply_feedbacks(&mut self) {
        for a in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            a.feedback = self.feedback;
        }
        for a in self.allpass2_l.iter_mut().chain(self.allpass2_r.iter_mut()) {
            a.feedback = self.feedback;
        }
    }

    fn apply_damp(&mut self) {
        for c in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            c.set_damp(self.damp);
        }
        for c in self.comb2_l.iter_mut().chain(self.comb2_r.iter_mut()) {
            c.set_damp(self.damp);
        }
    }

    fn set_rt60(&mut self, base: &Base, value: f64) {
        self.rt60 = value;
        let back = value * base.fs;
        let (back, zero) = if back > 0.0 {
            (back, 1.0f64)
        } else {
            (1.0, 0.0)
        };
        let fb = |size: usize| (zero * 10f64.powf(-3.0 * size as f64 / back)) as f32;
        for c in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            c.feedback = fb(c.size());
        }
        if self.nested {
            for c in self.comb2_l.iter_mut().chain(self.comb2_r.iter_mut()) {
                c.feedback = fb(c.size());
            }
        }
    }

    fn set_dc_cut(&mut self, base: &Base, v: f64) {
        self.dc_cut_fq = base.lim_fs2(v);
        self.in_dc.set_cut_freq(self.dc_cut_fq, base.fs);
        self.l_dc.set_cut_freq(self.dc_cut_fq, base.fs);
        self.r_dc.set_cut_freq(self.dc_cut_fq, base.fs);
    }

    fn set_damp_lpf(&mut self, base: &Base, v: f64) {
        self.damp_lpf = base.lim_fs2(v);
        self.damp_lpf_l.set_lowpass(self.damp_lpf, base.fs);
        self.damp_lpf_r.set_lowpass(self.damp_lpf, base.fs);
    }

    /// One frame of "Simple" (NRev with the Dampen lowpass), the wet signal only.
    #[inline]
    fn tick_simple(&mut self, base: &mut Base, in_l: f32, in_r: f32) -> (f32, f32) {
        self.hpf = (1.0 - self.damp3) * self.in_dc.process(in_l + in_r) - self.damp3 * self.hpf;
        self.hpf = super::prims::undenormal(self.hpf);
        let hpf = self.hpf * NREV_SCALE_WET;

        let mut out_l = 0.0;
        for c in &mut self.comb_l {
            out_l += c.process(hpf);
        }
        for a in &mut self.allpass_l[..3] {
            out_l = a.process_ov(out_l);
        }
        self.lpf_l = super::prims::undenormal(
            self.damp_lpf_l
                .process(self.damp2 * self.lpf_l + (1.0 - self.damp2) * out_l),
        );
        out_l = self.allpass_l[3].process_ov(self.lpf_l);
        out_l = self.allpass_l[5].process_ov(out_l);
        out_l = base.delay_wl.process(self.l_dc.process(out_l));

        let mut out_r = 0.0;
        for c in &mut self.comb_r {
            out_r += c.process(hpf);
        }
        for a in &mut self.allpass_r[..3] {
            out_r = a.process_ov(out_r);
        }
        self.lpf_r = super::prims::undenormal(
            self.damp_lpf_r
                .process(self.damp2 * self.lpf_r + (1.0 - self.damp2) * out_r),
        );
        out_r = self.allpass_r[3].process_ov(self.lpf_r);
        out_r = self.allpass_r[6].process_ov(out_r);
        out_r = base.delay_wr.process(self.r_dc.process(out_r));

        (
            out_l * base.wet1 + out_r * base.wet2,
            out_r * base.wet1 + out_l * base.wet2,
        )
    }

    /// One frame of "Nested" (NRevB with the Dampen lowpass).
    #[inline]
    fn tick_nested(&mut self, base: &mut Base, in_l: f32, in_r: f32) -> (f32, f32) {
        self.hpf = (1.0 - self.damp3) * self.in_dc.process(in_l + in_r) - self.damp3 * self.hpf;
        self.hpf = super::prims::undenormal(self.hpf);
        let tmp = self.hpf;
        let apf = self.ap_feedback;

        let mut out_l = tmp;
        out_l += apf * self.last_l;
        self.last_l += -apf * out_l;
        for c in &mut self.comb_l {
            out_l += c.process(tmp);
        }
        for c in &mut self.comb2_l {
            out_l += c.process(tmp);
        }
        for a in &mut self.allpass_l[..3] {
            out_l = a.process(out_l);
        }
        for a in &mut self.allpass2_l {
            out_l = a.process(out_l);
        }
        self.lpf_l = super::prims::undenormal(
            self.damp_lpf_l
                .process(self.damp2 * self.lpf_l + (1.0 - self.damp2) * out_l),
        );
        out_l = self.allpass_l[3].process(self.lpf_l);
        out_l = self.allpass_l[5].process(out_l);
        out_l = self.l_dc.process(out_l);

        let mut out_r = tmp;
        out_r += apf * self.last_r;
        self.last_r += -apf * out_r;
        for c in &mut self.comb_r {
            out_r += c.process(tmp);
        }
        for c in &mut self.comb2_r {
            out_r += c.process(tmp);
        }
        for a in &mut self.allpass_r[..3] {
            out_r = a.process(out_r);
        }
        for a in &mut self.allpass2_r {
            out_r = a.process(out_r);
        }
        self.lpf_r = super::prims::undenormal(
            self.damp_lpf_r
                .process(self.damp2 * self.lpf_r + (1.0 - self.damp2) * out_r),
        );
        out_r = self.allpass_r[3].process(self.lpf_r);
        // As in the original, the right side's last allpass is the left side's seventh.
        out_r = self.allpass_l[6].process(out_r);
        out_r = self.r_dc.process(out_r);

        let last_l = NREVB_SCALE_WET * base.delay_wl.process(self.last_l);
        let last_r = NREVB_SCALE_WET * base.delay_wr.process(self.last_r);
        let out = (
            last_l * base.wet1 + last_r * base.wet2,
            last_r * base.wet1 + last_l * base.wet2,
        );
        self.last_l = out_l;
        self.last_r = out_r;
        out
    }
}

/// `strev`: the tank plate.
#[derive(Clone)]
struct Strev {
    rt60: f64,
    decay: f32,
    dc_cut_fq: f64,
    input_damp: f64,
    damp: f64,
    output_damp: f64,
    diff1: f32,
    diff2: f32,
    idiff1: f32,
    idiff2: f32,
    spin: f64,
    spin_diff: f64,
    spin_limit: f64,
    wander: f32,
    mod_noise1: f32,
    mod_noise2: f32,
    auto_diff: bool,
    allpass_c: [AllPass; 4],
    allpass_c31: AllPass,
    allpass_c55: AllPass,
    allpass_m23: AllPassM,
    allpass_m46: AllPassM,
    delay_c30: Delay,
    delay_c39: Delay,
    delay_c54: Delay,
    delay_c63: Delay,
    dc: DcCut,
    lpf_in: Iir1,
    lpf_c30: Iir1,
    lpf_c54: Iir1,
    lfo1_lpf: Iir1,
    lfo2_lpf: Iir1,
    out_lpf: [Iir1; 2],
    lfo1: Lfo,
    lfo2: Lfo,
    noise: PinkNoise,
    il: [usize; 7],
    ir: [usize; 7],
    tank_delay: usize,
}

impl Strev {
    fn new() -> Self {
        Self {
            rt60: 1.0,
            decay: 1.0,
            dc_cut_fq: 6.0,
            input_damp: 10_000.0,
            damp: 4000.0,
            output_damp: 10_000.0,
            diff1: 0.7,
            diff2: 0.5,
            idiff1: 0.75,
            idiff2: 0.625,
            spin: 1.0,
            spin_diff: 0.15,
            spin_limit: 12.0,
            wander: 0.1,
            mod_noise1: 0.05,
            mod_noise2: 0.03,
            auto_diff: true,
            allpass_c: Default::default(),
            allpass_c31: AllPass::default(),
            allpass_c55: AllPass::default(),
            allpass_m23: AllPassM::default(),
            allpass_m46: AllPassM::default(),
            delay_c30: Delay::default(),
            delay_c39: Delay::default(),
            delay_c54: Delay::default(),
            delay_c63: Delay::default(),
            dc: DcCut::default(),
            lpf_in: Iir1::default(),
            lpf_c30: Iir1::default(),
            lpf_c54: Iir1::default(),
            lfo1_lpf: Iir1::default(),
            lfo2_lpf: Iir1::default(),
            out_lpf: [Iir1::default(); 2],
            lfo1: Lfo::default(),
            lfo2: Lfo::default(),
            noise: PinkNoise::default(),
            il: [1; 7],
            ir: [1; 7],
            tank_delay: 0,
        }
    }

    fn mute(&mut self) {
        for a in &mut self.allpass_c {
            a.mute();
        }
        self.allpass_c31.mute();
        self.allpass_c55.mute();
        self.allpass_m23.mute();
        self.allpass_m46.mute();
        self.delay_c30.mute();
        self.delay_c39.mute();
        self.delay_c54.mute();
        self.delay_c63.mute();
        self.dc.mute();
        for f in [
            &mut self.lpf_in,
            &mut self.lpf_c30,
            &mut self.lpf_c54,
            &mut self.lfo1_lpf,
            &mut self.lfo2_lpf,
        ] {
            f.mute();
        }
        for f in &mut self.out_lpf {
            f.mute();
        }
        self.lfo1.mute();
        self.lfo2.mute();
        self.noise.mute();
    }

    fn set_fs_factors(&mut self, base: &Base) {
        let total = base.total_factor_fs() / STREV_FS;
        let excur = base.fs / STREV_FS;
        let f = |d: f64| scaled(d, total);
        for i in 0..4 {
            self.allpass_c[i].set_size(f(STREV_ALLP[i]));
        }
        self.allpass_m23
            .set_size(f(STREV_ALLP[4]), scaled(STREV_EXCURSION, excur));
        self.allpass_m46
            .set_size(f(STREV_ALLP[5]), scaled(STREV_EXCURSION, excur));
        self.allpass_c31.set_size(f(STREV_ALLP[6]));
        self.allpass_c55.set_size(f(STREV_ALLP[7]));
        self.delay_c30.set_size(f(STREV_DELAY[0]));
        self.delay_c39.set_size(f(STREV_DELAY[1]));
        self.delay_c54.set_size(f(STREV_DELAY[2]));
        self.delay_c63.set_size(f(STREV_DELAY[3]));
        for i in 0..7 {
            self.il[i] = f(STREV_IDX_L[i]);
            self.ir[i] = f(STREV_IDX_R[i]);
        }
        self.tank_delay = (self.delay_c30.size()
            + self.delay_c39.size()
            + self.delay_c54.size()
            + self.delay_c63.size())
            / 4;
        self.set_rt60(base, self.rt60);
        self.set_dc_cut(base, self.dc_cut_fq);
        self.apply_diffusions();
        self.set_input_damp(base, self.input_damp);
        self.set_damp(base, self.damp);
        self.set_output_damp(base, self.output_damp);
        self.set_spin(base, self.spin);
        self.set_spin_limit(base, self.spin_limit);
    }

    fn apply_diffusions(&mut self) {
        self.allpass_c[0].feedback = -self.idiff1;
        self.allpass_c[1].feedback = -self.idiff1;
        self.allpass_c[2].feedback = -self.idiff2;
        self.allpass_c[3].feedback = -self.idiff2;
        self.allpass_c31.feedback = -self.diff1;
        self.allpass_c55.feedback = -self.diff1;
        self.allpass_m23.set_feedback(self.diff2);
        self.allpass_m46.set_feedback(self.diff2);
    }

    fn set_rt60(&mut self, base: &Base, value: f64) {
        self.rt60 = value;
        let back = value * base.fs;
        self.decay = if back > 0.0 {
            10f64.powf(-3.0 * self.tank_delay as f64 / back) as f32
        } else {
            0.0
        };
        if self.auto_diff {
            self.diff2 = (self.decay + 0.15).clamp(0.25, 0.5);
            self.apply_diffusions();
        }
    }

    fn set_dc_cut(&mut self, base: &Base, v: f64) {
        self.dc_cut_fq = base.lim_fs2(v);
        self.dc.set_cut_freq(self.dc_cut_fq, base.fs);
    }

    fn set_input_damp(&mut self, base: &Base, v: f64) {
        self.input_damp = base.lim_fs2(v);
        self.lpf_in.set_lowpass(self.input_damp, base.fs);
    }

    fn set_damp(&mut self, base: &Base, v: f64) {
        self.damp = base.lim_fs2(v);
        self.lpf_c30.set_lowpass(self.damp, base.fs);
        self.lpf_c54.set_lowpass(self.damp, base.fs);
    }

    fn set_output_damp(&mut self, base: &Base, v: f64) {
        self.output_damp = base.lim_fs2(v);
        for f in &mut self.out_lpf {
            f.set_lowpass(self.output_damp, base.fs);
        }
    }

    fn set_spin(&mut self, base: &Base, v: f64) {
        if v > 0.0 {
            self.spin = base.lim_fs2(v);
            self.lfo1.set_freq(self.spin / base.fs);
            self.lfo2.set_freq((self.spin + self.spin_diff) / base.fs);
        } else {
            self.spin = 0.0;
            self.lfo1.set_freq(0.0);
            self.lfo2.set_freq(0.0);
        }
    }

    fn set_spin_limit(&mut self, base: &Base, v: f64) {
        self.spin_limit = base.lim_fs2(v);
        self.lfo1_lpf.set_lowpass(self.spin_limit, base.fs);
        self.lfo2_lpf.set_lowpass(self.spin_limit, base.fs);
    }

    #[inline]
    fn tick(&mut self, base: &mut Base, in_l: f32, in_r: f32) -> (f32, f32) {
        let input = (in_l + in_r) / 2.0;
        let mut input = self.lpf_in.process(self.dc.process(input));
        for a in &mut self.allpass_c {
            input = a.process(input);
        }
        let out_l = input + self.delay_c63.last() * self.decay;
        let out_r = input + self.delay_c39.last() * self.decay;

        let noise = self.noise.process();
        let lfo1q = self
            .lfo1_lpf
            .process((self.lfo1.process() + self.mod_noise1 * noise) * self.wander);
        let lfo2q = self
            .lfo2_lpf
            .process((self.lfo2.process() - self.mod_noise1 * noise) * self.wander);

        let a = self
            .allpass_m23
            .process_fmod(out_l, lfo1q, self.mod_noise2 * noise);
        let a = self.lpf_c30.process(self.delay_c30.process(a));
        let a = self.allpass_c31.process(self.decay * a);
        self.delay_c39.process(a);
        let b = self
            .allpass_m46
            .process_fmod(out_r, lfo2q, -self.mod_noise2 * noise);
        let b = self.lpf_c54.process(self.delay_c54.process(b));
        let b = self.allpass_c55.process(self.decay * b);
        self.delay_c63.process(b);

        let (il, ir) = (&self.il, &self.ir);
        let out_l = self.delay_c54.get_z(il[0]) + self.delay_c54.get_z(il[1])
            - self.allpass_c55.get_z(il[2])
            + self.delay_c63.get_z(il[3])
            - self.delay_c30.get_z(il[4])
            - self.allpass_c31.get_z(il[5])
            - self.delay_c39.get_z(il[6]);
        let out_r = self.delay_c30.get_z(ir[0]) + self.delay_c30.get_z(ir[1])
            - self.allpass_c31.get_z(ir[2])
            + self.delay_c39.get_z(ir[3])
            - self.delay_c54.get_z(ir[4])
            - self.allpass_c55.get_z(ir[5])
            - self.delay_c63.get_z(ir[6]);

        let fp_l = base.delay_wl.process(self.out_lpf[0].process(out_l));
        let fp_r = base.delay_wr.process(self.out_lpf[1].process(out_r));
        (
            fp_l * base.wet1 + fp_r * base.wet2,
            fp_r * base.wet1 + fp_l * base.wet2,
        )
    }
}

/// The Plate: all three algorithms built once, one running at a time, behind the input low and high cut.
#[derive(Clone)]
pub struct Plate {
    base: Base,
    algorithm: Algorithm,
    simple: NRev,
    nested: NRev,
    tank: Strev,
    in_lpf: [Iir1; 2],
    in_hpf: [Iir1; 2],
    in_lpf_hz: f64,
    in_hpf_hz: f64,
}

impl Plate {
    pub fn new(fs: f64) -> Self {
        let mut p = Self {
            base: Base::new(),
            algorithm: Algorithm::Nested,
            simple: NRev::new(false),
            nested: NRev::new(true),
            tank: Strev::new(),
            in_lpf: [Iir1::default(); 2],
            in_hpf: [Iir1::default(); 2],
            in_lpf_hz: 16_000.0,
            in_hpf_hz: 0.0,
        };
        p.base.fs = fs;
        p.base.reserve_pre_delay(MAX_PRE_DELAY_MS, MAX_FS);
        p.base.set_pre_delay(0.1);
        p.base.set_wet_db(0.0);
        p.set_fs_factors();
        p.mute();
        p
    }

    fn set_fs_factors(&mut self) {
        let predelay = self.base.pre_delay();
        self.base.set_pre_delay(predelay);
        self.simple.set_fs_factors(&self.base);
        self.nested.set_fs_factors(&self.base);
        self.tank.set_fs_factors(&self.base);
        self.set_input_lpf(self.in_lpf_hz);
        self.set_input_hpf(self.in_hpf_hz);
    }

    pub fn mute(&mut self) {
        self.base.mute();
        self.simple.mute();
        self.nested.mute();
        self.tank.mute();
        for f in self.in_lpf.iter_mut().chain(self.in_hpf.iter_mut()) {
            f.mute();
        }
    }

    pub fn set_sample_rate(&mut self, fs: f64) {
        if fs > 0.0 {
            self.base.fs = fs;
            self.set_fs_factors();
            self.mute();
        }
    }

    /// Changing the algorithm silences the one that was running, as the plugin does.
    pub fn set_algorithm(&mut self, algorithm: Algorithm) {
        if algorithm != self.algorithm {
            match self.algorithm {
                Algorithm::Simple => self.simple.mute(),
                Algorithm::Nested => self.nested.mute(),
                Algorithm::Tank => self.tank.mute(),
            }
            self.base.mute();
            self.algorithm = algorithm;
        }
    }

    pub fn set_width(&mut self, w: f32) {
        self.base.set_width(w);
    }

    pub fn set_pre_delay(&mut self, ms: f64) {
        self.base.set_pre_delay(ms.max(0.1));
    }

    pub fn set_rt60(&mut self, s: f64) {
        self.simple.set_rt60(&self.base, s);
        self.nested.set_rt60(&self.base, s);
        self.tank.set_rt60(&self.base, s);
    }

    /// Dampen: the lowpass in each algorithm's loop.
    pub fn set_damp(&mut self, hz: f64) {
        self.simple.set_damp_lpf(&self.base, hz);
        self.nested.set_damp_lpf(&self.base, hz);
        self.tank.set_damp(&self.base, hz);
        self.tank
            .set_output_damp(&self.base, (hz * 2.0).max(16_000.0));
    }

    pub fn set_input_lpf(&mut self, hz: f64) {
        self.in_lpf_hz = hz.clamp(0.0, self.base.fs / 2.0);
        for f in &mut self.in_lpf {
            f.set_lowpass(self.in_lpf_hz, self.base.fs);
        }
    }

    pub fn set_input_hpf(&mut self, hz: f64) {
        self.in_hpf_hz = hz.clamp(0.0, self.base.fs / 2.0);
        for f in &mut self.in_hpf {
            f.set_highpass(self.in_hpf_hz, self.base.fs);
        }
    }

    /// One stereo frame in, the plate's wet signal out (the host mixes the dry).
    #[inline]
    pub fn tick(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let l = self.in_lpf[0].process(self.in_hpf[0].process(in_l));
        let r = self.in_lpf[1].process(self.in_hpf[1].process(in_r));
        match self.algorithm {
            Algorithm::Simple => self.simple.tick_simple(&mut self.base, l, r),
            Algorithm::Nested => self.nested.tick_nested(&mut self.base, l, r),
            Algorithm::Tank => self.tank.tick(&mut self.base, l, r),
        }
    }
}
