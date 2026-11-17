//! The reverb building blocks: delay lines, allpass and comb filters, one- and two-pole filters, an LFO and a DC
//! cut. Ported from Freeverb3 (Teru Kamogashira, GPL-2.0-or-later), as used by Dragonfly Reverb (Michael Willis and
//! Rob van den Berg, GPL-3.0-or-later).
//!
//! Every buffer is a `Vec` that only grows when a structure is first sized: a reverb is built once at its largest
//! size, so changing a size later (Size, Wander, Predelay) re-uses the memory and the audio thread never allocates.

use std::f64::consts::{LN_2, PI};

/// A sample that is neither zero nor normal (a denormal, infinity or NaN) is zero.
#[inline]
pub fn undenormal(v: f32) -> f32 {
    if v == 0.0 || v.is_normal() { v } else { 0.0 }
}

pub fn is_prime(n: i64) -> bool {
    if n == 2 {
        return true;
    }
    if n & 1 == 0 || n < 2 {
        return false;
    }
    let limit = (n as f64).sqrt() as i64 + 1;
    (3..limit).step_by(2).all(|i| n % i != 0)
}

pub fn db_to_ratio(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub fn ratio_to_db(r: f64) -> f64 {
    20.0 * r.ln() / 10f64.ln()
}

/// `f_`: a design length times a factor, in samples, at least 1.
pub fn scaled(def: f64, factor: f64) -> usize {
    ((def * factor) as i64).max(1) as usize
}

/// `p_`: [`scaled`] moved up to the next prime, so the loops of a reverb do not ring together.
pub fn prime_scaled(def: f64, factor: f64) -> usize {
    let mut n = scaled(def, factor) as i64;
    while !is_prime(n) {
        n += 1;
    }
    n as usize
}

/// Zeroes `buf` and gives it `size` samples (no allocation within its capacity).
fn zeroed(buf: &mut Vec<f32>, size: usize) {
    buf.clear();
    buf.resize(size, 0.0);
}

/// A plain delay of `size` samples (`size` 0 passes the input through). Resizing keeps what was in it.
#[derive(Clone, Default)]
pub struct Delay {
    buf: Vec<f32>,
    idx: usize,
}

impl Delay {
    pub fn size(&self) -> usize {
        self.buf.len()
    }

    /// Room for `n` samples, so a later `set_size` within it does not allocate.
    pub fn reserve(&mut self, n: usize) {
        self.buf.reserve(n.saturating_sub(self.buf.len()));
    }

    /// A size of 0 changes nothing, as in Freeverb3. Growing puts silence in front of what was there, shrinking
    /// drops its oldest samples.
    pub fn set_size(&mut self, size: usize) {
        if size == 0 {
            return;
        }
        let old = self.buf.len();
        if old == 0 {
            zeroed(&mut self.buf, size);
            self.idx = 0;
            return;
        }
        self.buf.rotate_left(self.idx);
        if size >= old {
            self.buf.resize(size, 0.0);
            self.buf.rotate_right(size - old);
        } else {
            let cut = old - size;
            self.buf.copy_within(cut..old, 0);
            self.buf.truncate(size);
        }
        self.idx = 0;
    }

    pub fn mute(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let out = self.buf[self.idx];
        self.buf[self.idx] = input;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    /// What the next `process` returns (the oldest sample).
    #[inline]
    pub fn last(&self) -> f32 {
        self.buf.get(self.idx).copied().unwrap_or(0.0)
    }

    /// The sample written `index` steps ago (1 = the latest), `index` kept inside 1 … size.
    #[inline]
    pub fn get_z(&self, index: usize) -> f32 {
        let n = self.buf.len();
        if n == 0 {
            return 0.0;
        }
        let index = index.clamp(1, n);
        let at = if self.idx >= index {
            self.idx - index
        } else {
            self.idx + n - index
        };
        self.buf[at]
    }
}

/// A delay whose read point wanders: `modulation` (−1 … 1) moves it by up to its modulation size either way, with
/// an allpass interpolation. `process` returns the delayed sample and `last` repeats it. `feedback` scales what is
/// written.
#[derive(Clone, Default)]
pub struct DelayM {
    buf: Vec<f32>,
    read: usize,
    write: usize,
    mod_size: usize,
    mod_f: f32,
    z1: f32,
    pub feedback: f32,
}

impl DelayM {
    pub fn size(&self) -> usize {
        self.buf.len()
    }

    /// The length without the modulation room.
    pub fn delay_size(&self) -> usize {
        self.buf.len() - self.mod_size
    }

    pub fn set_size(&mut self, size: usize, mod_size: usize) {
        if size == 0 {
            return;
        }
        let mod_size = mod_size.min(size);
        zeroed(&mut self.buf, size + mod_size);
        self.read = mod_size * 2;
        self.write = 0;
        self.mod_size = mod_size;
        self.mod_f = mod_size as f32;
        self.z1 = 0.0;
    }

    pub fn mute(&mut self) {
        self.buf.fill(0.0);
        self.write = 0;
        self.z1 = 0.0;
        self.read = self.mod_size * 2;
    }

    #[inline]
    pub fn last(&self) -> f32 {
        self.z1
    }

    #[inline]
    pub fn process(&mut self, input: f32, modulation: f32) -> f32 {
        let n = self.buf.len();
        if n == 0 {
            return input;
        }
        let modulation = (modulation + 1.0) * self.mod_f;
        let floor = modulation.floor();
        let frac = 1.0 - (modulation - floor);
        let mut a = self.read as i64 - floor as i64;
        if a < 0 {
            a += n as i64;
        }
        let mut b = a - 1;
        if b < 0 {
            b += n as i64;
        }
        self.z1 = undenormal(self.buf[b as usize] + frac * (self.buf[a as usize] - self.z1));
        self.read += 1;
        if self.read >= n {
            self.read = 0;
        }
        self.buf[self.write] = self.feedback * input;
        self.write += 1;
        if self.write >= n {
            self.write = 0;
        }
        self.z1
    }
}

/// A Schroeder allpass: `feedback` is its gain.
#[derive(Clone)]
pub struct AllPass {
    buf: Vec<f32>,
    idx: usize,
    pub feedback: f32,
    pub decay: f32,
}

impl Default for AllPass {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            idx: 0,
            feedback: 0.0,
            decay: 1.0,
        }
    }
}

impl AllPass {
    pub fn size(&self) -> usize {
        self.buf.len()
    }

    pub fn set_size(&mut self, size: usize) {
        if size == 0 {
            return;
        }
        zeroed(&mut self.buf, size);
        self.idx = 0;
    }

    pub fn mute(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let stored = self.buf[self.idx];
        let input = input + self.feedback * stored;
        let out = undenormal(stored - self.feedback * input);
        self.buf[self.idx] = input;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    /// Freeverb's overlapped allpass (the Plate's "Simple" algorithm).
    #[inline]
    pub fn process_ov(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let stored = undenormal(self.buf[self.idx]);
        self.buf[self.idx] = input + stored * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        stored - input
    }

    /// The sample written `index` steps ago (1 = the latest), kept inside 1 … size.
    #[inline]
    pub fn get_z(&self, index: usize) -> f32 {
        z_tap(&self.buf, self.idx, index)
    }

    /// As `process`, with the stored signal scaled by `decay` on the way out (a loss in the allpass).
    #[inline]
    pub fn process_dc(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let stored = self.buf[self.idx];
        let input = input + self.feedback * stored;
        let out = undenormal(self.decay * stored - self.feedback * input);
        self.buf[self.idx] = input;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }
}

/// An allpass with a wandering read point (see [`DelayM`]). `process` takes the modulation (−1 … 1) and, optionally,
/// an offset to the feedback for that sample; `decay` scales the delayed signal where `process_dc` returns it.
#[derive(Clone)]
pub struct AllPassM {
    buf: Vec<f32>,
    read: usize,
    write: usize,
    mod_size: usize,
    mod_f: f32,
    z1: f32,
    feedback: f32,
    feedback_mod: f32,
    pub decay: f32,
}

impl Default for AllPassM {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            read: 0,
            write: 0,
            mod_size: 0,
            mod_f: 0.0,
            z1: 0.0,
            feedback: 0.0,
            feedback_mod: 0.0,
            decay: 1.0,
        }
    }
}

impl AllPassM {
    pub fn size(&self) -> usize {
        self.buf.len()
    }

    pub fn delay_size(&self) -> usize {
        self.buf.len() - self.mod_size
    }

    pub fn set_size(&mut self, size: usize, mod_size: usize) {
        if size == 0 {
            return;
        }
        let mod_size = mod_size.min(size);
        zeroed(&mut self.buf, size + mod_size);
        self.read = mod_size * 2;
        self.write = 0;
        self.mod_size = mod_size;
        self.mod_f = mod_size as f32;
        self.z1 = 0.0;
    }

    pub fn set_feedback(&mut self, v: f32) {
        self.feedback = v;
        self.feedback_mod = v;
    }

    pub fn mute(&mut self) {
        self.buf.fill(0.0);
        self.write = 0;
        self.z1 = 0.0;
        self.read = self.mod_size * 2;
        self.feedback_mod = self.feedback;
    }

    /// The interpolated read: the delayed sample, the read point moved on.
    #[inline]
    fn read_delayed(&mut self, modulation: f32) {
        let n = self.buf.len();
        let modulation = (modulation + 1.0) * self.mod_f;
        let floor = modulation.floor();
        let frac = 1.0 - (modulation - floor);
        let mut a = self.read as i64 - floor as i64;
        if a < 0 {
            a += n as i64;
        }
        let mut b = a - 1;
        if b < 0 {
            b += n as i64;
        }
        self.z1 = undenormal(self.buf[b as usize] + frac * (self.buf[a as usize] - self.z1));
        self.read += 1;
        if self.read >= n {
            self.read = 0;
        }
    }

    #[inline]
    fn write_back(&mut self, input: f32, gain: f32) -> f32 {
        let written = input + self.z1 * self.feedback_mod;
        self.buf[self.write] = written;
        let out = gain * self.z1 - written * self.feedback_mod;
        self.write += 1;
        if self.write >= self.buf.len() {
            self.write = 0;
        }
        out
    }

    #[inline]
    pub fn process(&mut self, input: f32, modulation: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        self.read_delayed(modulation);
        self.write_back(input, 1.0)
    }

    /// With `fmod` added to the feedback for this sample.
    #[inline]
    pub fn process_fmod(&mut self, input: f32, modulation: f32, fmod: f32) -> f32 {
        self.feedback_mod = self.feedback + fmod;
        self.process(input, modulation)
    }

    /// With the delayed signal scaled by `decay` on the way out.
    #[inline]
    pub fn process_dc(&mut self, input: f32, modulation: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        self.read_delayed(modulation);
        self.write_back(input, self.decay)
    }

    #[inline]
    pub fn process_dc_fmod(&mut self, input: f32, modulation: f32, fmod: f32) -> f32 {
        self.feedback_mod = self.feedback + fmod;
        self.process_dc(input, modulation)
    }
}

/// Two allpasses in a row sharing one structure (Freeverb3's `allpass2`): `decay1`/`decay2` scale each one's delayed
/// signal, and the taps at `get_z1`/`get_z2` are read by the Room's outputs.
#[derive(Clone)]
pub struct AllPass2 {
    buf1: Vec<f32>,
    buf2: Vec<f32>,
    idx1: usize,
    idx2: usize,
    pub feedback1: f32,
    pub feedback2: f32,
    pub decay1: f32,
    pub decay2: f32,
}

impl Default for AllPass2 {
    fn default() -> Self {
        Self {
            buf1: Vec::new(),
            buf2: Vec::new(),
            idx1: 0,
            idx2: 0,
            feedback1: 0.0,
            feedback2: 0.0,
            decay1: 1.0,
            decay2: 1.0,
        }
    }
}

impl AllPass2 {
    pub fn set_size(&mut self, size1: usize, size2: usize) {
        if size1 == 0 || size2 == 0 {
            return;
        }
        zeroed(&mut self.buf1, size1);
        zeroed(&mut self.buf2, size2);
        self.idx1 = 0;
        self.idx2 = 0;
    }

    pub fn mute(&mut self) {
        self.buf1.fill(0.0);
        self.buf2.fill(0.0);
        self.idx1 = 0;
        self.idx2 = 0;
    }

    #[inline]
    pub fn get_z1(&self, index: usize) -> f32 {
        z_tap(&self.buf1, self.idx1, index)
    }

    #[inline]
    pub fn get_z2(&self, index: usize) -> f32 {
        z_tap(&self.buf2, self.idx2, index)
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.buf1.is_empty() || self.buf2.is_empty() {
            return input;
        }
        let (i1, i2) = (self.idx1, self.idx2);
        let mut input = input + self.feedback2 * self.buf2[i2];
        let out = undenormal(self.decay2 * self.buf2[i2] - input * self.feedback2);
        input += self.feedback1 * self.buf1[i1];
        self.buf2[i2] = undenormal(self.decay1 * self.buf1[i1] - input * self.feedback1);
        self.buf1[i1] = input;
        self.idx1 += 1;
        if self.idx1 >= self.buf1.len() {
            self.idx1 = 0;
        }
        self.idx2 += 1;
        if self.idx2 >= self.buf2.len() {
            self.idx2 = 0;
        }
        out
    }
}

/// Three allpasses in a row, the first with a wandering read (Freeverb3's `allpass3`).
#[derive(Clone)]
pub struct AllPass3 {
    buf1: Vec<f32>,
    buf2: Vec<f32>,
    buf3: Vec<f32>,
    read1: usize,
    write1: usize,
    idx2: usize,
    idx3: usize,
    mod_f: f32,
    mod_size: usize,
    pub feedback1: f32,
    pub feedback2: f32,
    pub feedback3: f32,
    pub decay1: f32,
    pub decay2: f32,
    pub decay3: f32,
}

impl Default for AllPass3 {
    fn default() -> Self {
        Self {
            buf1: Vec::new(),
            buf2: Vec::new(),
            buf3: Vec::new(),
            read1: 0,
            write1: 0,
            idx2: 0,
            idx3: 0,
            mod_f: 0.0,
            mod_size: 0,
            feedback1: 0.0,
            feedback2: 0.0,
            feedback3: 0.0,
            decay1: 1.0,
            decay2: 1.0,
            decay3: 1.0,
        }
    }
}

impl AllPass3 {
    pub fn set_size(&mut self, size1: usize, size1_mod: usize, size2: usize, size3: usize) {
        if size1 == 0 || size2 == 0 || size3 == 0 {
            return;
        }
        let size1_mod = size1_mod.min(size1);
        zeroed(&mut self.buf1, size1 + size1_mod);
        zeroed(&mut self.buf2, size2);
        zeroed(&mut self.buf3, size3);
        self.mod_size = size1_mod;
        self.mod_f = size1_mod as f32;
        self.idx2 = 0;
        self.idx3 = 0;
        self.write1 = 0;
        self.read1 = size1_mod * 2;
    }

    pub fn mute(&mut self) {
        self.buf1.fill(0.0);
        self.buf2.fill(0.0);
        self.buf3.fill(0.0);
        self.idx2 = 0;
        self.idx3 = 0;
        self.write1 = 0;
        self.read1 = self.mod_size * 2;
    }

    #[inline]
    pub fn get_z1(&self, index: usize) -> f32 {
        z_tap(&self.buf1, self.read1, index)
    }

    #[inline]
    pub fn get_z2(&self, index: usize) -> f32 {
        z_tap(&self.buf2, self.idx2, index)
    }

    #[inline]
    pub fn get_z3(&self, index: usize) -> f32 {
        z_tap(&self.buf3, self.idx3, index)
    }

    /// With no modulation (the first stage reads at its plain read point).
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.buf1.is_empty() || self.buf2.is_empty() || self.buf3.is_empty() {
            return input;
        }
        let first = self.buf1[self.read1];
        self.run(input, first)
    }

    /// With the first stage's read point moved by `modulation` (−1 … 1), interpolated linearly.
    #[inline]
    pub fn process_mod(&mut self, input: f32, modulation: f32) -> f32 {
        if self.buf1.is_empty() || self.buf2.is_empty() || self.buf3.is_empty() {
            return input;
        }
        let n = self.buf1.len() as i64;
        let modulation = (modulation + 1.0) * self.mod_f;
        let floor = modulation.floor();
        let frac = modulation - floor;
        let mut a = self.read1 as i64 - floor as i64;
        if a < 0 {
            a += n;
        }
        let mut b = a - 1;
        if b < 0 {
            b += n;
        }
        let first = self.buf1[b as usize] * frac + self.buf1[a as usize] * (1.0 - frac);
        self.run(input, first)
    }

    #[inline]
    fn run(&mut self, input: f32, first: f32) -> f32 {
        let (i2, i3) = (self.idx2, self.idx3);
        let mut input = input + self.feedback3 * self.buf3[i3];
        let out = undenormal(self.decay3 * self.buf3[i3] - self.feedback3 * input);
        input += self.feedback2 * self.buf2[i2];
        self.buf3[i3] = undenormal(self.decay2 * self.buf2[i2] - self.feedback2 * input);
        input += self.feedback1 * first;
        self.buf2[i2] = undenormal(self.decay1 * first - self.feedback1 * input);
        self.buf1[self.write1] = input;
        let n1 = self.buf1.len();
        self.write1 += 1;
        if self.write1 >= n1 {
            self.write1 = 0;
        }
        self.read1 += 1;
        if self.read1 >= n1 {
            self.read1 = 0;
        }
        self.idx2 += 1;
        if self.idx2 >= self.buf2.len() {
            self.idx2 = 0;
        }
        self.idx3 += 1;
        if self.idx3 >= self.buf3.len() {
            self.idx3 = 0;
        }
        out
    }
}

/// The sample `index` steps before `at` in a ring buffer.
#[inline]
fn z_tap(buf: &[f32], at: usize, index: usize) -> f32 {
    let n = buf.len();
    if n == 0 {
        return 0.0;
    }
    let index = index.clamp(1, n);
    buf[if at >= index {
        at - index
    } else {
        at + n - index
    }]
}

/// A lowpass-feedback comb (Freeverb's): `damp` is the lowpass in the loop, `feedback` the loop gain.
#[derive(Clone, Default)]
pub struct Comb {
    buf: Vec<f32>,
    idx: usize,
    pub feedback: f32,
    store: f32,
    damp1: f32,
    damp2: f32,
}

impl Comb {
    pub fn new() -> Self {
        Self {
            damp2: 1.0,
            ..Self::default()
        }
    }

    pub fn size(&self) -> usize {
        self.buf.len()
    }

    /// Resizing keeps what was in it, as a delay does.
    pub fn set_size(&mut self, size: usize) {
        if size == 0 {
            return;
        }
        let old = self.buf.len();
        if old == 0 {
            zeroed(&mut self.buf, size);
        } else {
            self.buf.rotate_left(self.idx);
            if size >= old {
                self.buf.resize(size, 0.0);
                self.buf.rotate_right(size - old);
            } else {
                let cut = old - size;
                self.buf.copy_within(cut..old, 0);
                self.buf.truncate(size);
            }
        }
        self.idx = 0;
        self.store = 0.0;
    }

    pub fn set_damp(&mut self, v: f32) {
        self.damp1 = v;
        self.damp2 = 1.0 - v;
    }

    pub fn mute(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
        self.store = 0.0;
    }

    /// Freeverb's lowpass comb.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let out = undenormal(self.buf[self.idx]);
        self.store = out * self.damp2 + self.store * self.damp1;
        self.buf[self.idx] = input + self.store * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    /// The feed-forward comb: `input + feedback · delayed(input)`.
    #[inline]
    pub fn process_ff(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let out = self.buf[self.idx] * self.feedback + input;
        self.buf[self.idx] = input;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        undenormal(out)
    }
}

/// A second-order filter (direct form I), set by the RBJ cookbook formulas.
#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    i1: f32,
    i2: f32,
    o1: f32,
    o2: f32,
}

impl Biquad {
    pub fn mute(&mut self) {
        self.i1 = 0.0;
        self.i2 = 0.0;
        self.o1 = 0.0;
        self.o2 = 0.0;
    }

    fn coefficients(&mut self, b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) {
        self.b0 = b0 as f32;
        self.b1 = b1 as f32;
        self.b2 = b2 as f32;
        self.a1 = a1 as f32;
        self.a2 = a2 as f32;
    }

    /// The RBJ alpha for a bandwidth in octaves.
    fn alpha_bw(fc: f64, bw: f64, fs: f64) -> f64 {
        let omega = 2.0 * PI * fc / fs;
        let sn = omega.sin();
        sn * (LN_2 / 2.0 * bw * omega / sn).sinh()
    }

    /// An allpass at `fc` with bandwidth `bw` octaves.
    pub fn set_allpass_bw(&mut self, fc: f64, bw: f64, fs: f64) {
        let omega = 2.0 * PI * fc / fs;
        let cs = omega.cos();
        let alpha = Self::alpha_bw(fc, bw, fs);
        let a0r = 1.0 / (1.0 + alpha);
        self.coefficients(
            a0r * (1.0 - alpha),
            a0r * (-2.0 * cs),
            a0r * (1.0 + alpha),
            a0r * (-2.0 * cs),
            a0r * (1.0 - alpha),
        );
    }

    /// A lowpass at `fc` with bandwidth `bw` octaves.
    pub fn set_lowpass_bw(&mut self, fc: f64, bw: f64, fs: f64) {
        let omega = 2.0 * PI * fc / fs;
        let cs = omega.cos();
        let alpha = Self::alpha_bw(fc, bw, fs);
        let a0r = 1.0 / (1.0 + alpha);
        self.coefficients(
            a0r * (1.0 - cs) * 0.5,
            a0r * (1.0 - cs),
            a0r * (1.0 - cs) * 0.5,
            a0r * (-2.0 * cs),
            a0r * (1.0 - alpha),
        );
    }

    /// A low shelf: `gain` dB, `slope` 0…1.
    pub fn set_low_shelf(&mut self, fc: f64, gain: f64, slope: f64, fs: f64) {
        let w = 2.0 * PI * fc.clamp(1.0, fs / 2.0) / fs;
        let (cw, sw) = (w.cos(), w.sin());
        let a = 10f64.powf(gain * 0.025);
        let b = (((1.0 + a * a) / slope.clamp(0.0001, 1.0)) - (a - 1.0) * (a - 1.0)).sqrt();
        let apc = cw * (a + 1.0);
        let amc = cw * (a - 1.0);
        let bs = b * sw;
        let a0r = 1.0 / (a + 1.0 + amc + bs);
        self.coefficients(
            a0r * a * (a + 1.0 - amc + bs),
            a0r * 2.0 * a * (a - 1.0 - apc),
            a0r * a * (a + 1.0 - amc - bs),
            -a0r * 2.0 * (a - 1.0 + apc),
            -a0r * (-a - 1.0 - amc + bs),
        );
    }

    /// A high shelf: `gain` dB, `slope` 0…1.
    pub fn set_high_shelf(&mut self, fc: f64, gain: f64, slope: f64, fs: f64) {
        let w = 2.0 * PI * fc.clamp(1.0, fs / 2.0) / fs;
        let (cw, sw) = (w.cos(), w.sin());
        let a = 10f64.powf(gain * 0.025);
        let b = (((1.0 + a * a) / slope.clamp(0.0001, 1.0)) - (a - 1.0) * (a - 1.0)).sqrt();
        let apc = cw * (a + 1.0);
        let amc = cw * (a - 1.0);
        let bs = b * sw;
        let a0r = 1.0 / (a + 1.0 - amc + bs);
        self.coefficients(
            a0r * a * (a + 1.0 + amc + bs),
            a0r * -2.0 * a * (a - 1.0 + apc),
            a0r * a * (a + 1.0 + amc - bs),
            -a0r * -2.0 * (a - 1.0 - apc),
            -a0r * (-a - 1.0 + amc + bs),
        );
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let x = input;
        let mut y = input * self.b0;
        y += self.b1 * self.i1 + self.b2 * self.i2;
        y -= self.a1 * self.o1 + self.a2 * self.o2;
        y = undenormal(y);
        self.i2 = self.i1;
        self.i1 = x;
        self.o2 = self.o1;
        self.o1 = y;
        y
    }
}

/// A first-order filter.
#[derive(Clone, Copy)]
pub struct Iir1 {
    a2: f32,
    b1: f32,
    b2: f32,
    y1: f32,
}

impl Default for Iir1 {
    /// A pass-through.
    fn default() -> Self {
        Self {
            a2: 0.0,
            b1: 1.0,
            b2: 0.0,
            y1: 0.0,
        }
    }
}

impl Iir1 {
    pub fn mute(&mut self) {
        self.y1 = 0.0;
    }

    /// A Butterworth lowpass at `fc`.
    pub fn set_lowpass(&mut self, fc: f64, fs: f64) {
        let t = (PI * fc / fs).tan();
        self.b1 = (t / (1.0 + t)) as f32;
        self.b2 = self.b1;
        self.a2 = ((1.0 - t) / (1.0 + t)) as f32;
    }

    /// A Butterworth highpass at `fc`.
    pub fn set_highpass(&mut self, fc: f64, fs: f64) {
        let t = (PI * fc / fs).tan();
        self.b1 = (1.0 / (1.0 + t)) as f32;
        self.b2 = -self.b1;
        self.a2 = ((1.0 - t) / (1.0 + t)) as f32;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let out = undenormal(input * self.b1 + self.y1);
        self.y1 = undenormal(out * self.a2 + input * self.b2);
        out
    }
}

/// A sine as a rotating phasor, renormalised now and then: cheap and drift-free.
#[derive(Clone, Copy)]
pub struct Lfo {
    re: f32,
    im: f32,
    arc_re: f32,
    arc_im: f32,
    count: u32,
}

/// How many steps a phasor runs between renormalisations.
const LFO_RENORM: u32 = 10_000;

impl Default for Lfo {
    fn default() -> Self {
        Self {
            re: 1.0,
            im: 0.0,
            arc_re: 1.0,
            arc_im: 0.0,
            count: 0,
        }
    }
}

impl Lfo {
    pub fn mute(&mut self) {
        self.re = 1.0;
        self.im = 0.0;
        self.count = 0;
    }

    /// The frequency in cycles per sample.
    pub fn set_freq(&mut self, fc: f64) {
        let theta = 2.0 * PI * fc;
        self.arc_re = theta.cos() as f32;
        self.arc_im = theta.sin() as f32;
    }

    #[inline]
    pub fn process(&mut self) -> f32 {
        let out = self.im;
        let re = undenormal(self.re * self.arc_re - self.im * self.arc_im);
        let im = undenormal(self.re * self.arc_im + self.im * self.arc_re);
        self.re = re;
        self.im = im;
        self.count += 1;
        if self.count > LFO_RENORM {
            self.count = 0;
            let r = undenormal((self.re * self.re + self.im * self.im).sqrt());
            if r > 0.0 {
                self.re /= r;
                self.im /= r;
            }
        }
        out.clamp(-1.0, 1.0)
    }
}

/// A DC blocker.
#[derive(Clone, Copy)]
pub struct DcCut {
    gain: f32,
    y1: f32,
    y2: f32,
}

impl Default for DcCut {
    fn default() -> Self {
        Self {
            gain: 0.9999,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl DcCut {
    pub fn mute(&mut self) {
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    pub fn set_cut_freq(&mut self, fc: f64, fs: f64) {
        let f = 2.0 * fc / fs;
        let s = (PI * f).sin();
        self.gain = ((3f64.sqrt() - 2.0 * s) / (s + 3f64.sqrt() * (PI * f).cos())) as f32;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let mut out = input;
        out -= self.y1;
        self.y1 = input;
        out += self.gain * self.y2;
        self.y2 = out;
        undenormal(out)
    }
}

/// What every reverb here shares (Freeverb3's `revbase`): sample rate and size factor, the predelay, the wet and dry
/// levels and the stereo width.
#[derive(Clone)]
pub struct Base {
    pub fs: f64,
    pub rs_factor: f64,
    pre_delay_ms: f64,
    pub delay_wl: Delay,
    pub delay_wr: Delay,
    pub wet: f32,
    pub wet1: f32,
    pub wet2: f32,
    pub dry: f32,
    pub width: f32,
}

impl Base {
    pub fn new() -> Self {
        let mut b = Self {
            fs: 48_000.0,
            rs_factor: 1.0,
            pre_delay_ms: 0.0,
            delay_wl: Delay::default(),
            delay_wr: Delay::default(),
            wet: 1.0,
            wet1: 1.0,
            wet2: 0.0,
            dry: 1.0,
            width: 1.0,
        };
        b.set_width(1.0);
        b
    }

    /// The sample rate times the size factor: what the delay lengths scale by.
    pub fn total_factor_fs(&self) -> f64 {
        self.fs * self.rs_factor
    }

    pub fn set_wet_db(&mut self, db: f64) {
        self.wet = db_to_ratio(db) as f32;
        self.update_wet();
    }

    pub fn set_dry_ratio(&mut self, r: f32) {
        self.dry = r;
    }

    pub fn set_width(&mut self, w: f32) {
        self.width = w;
        self.update_wet();
    }

    fn update_wet(&mut self) {
        self.wet1 = self.wet * (self.width / 2.0 + 0.5);
        self.wet2 = self.wet * ((1.0 - self.width) / 2.0);
    }

    pub fn set_pre_delay(&mut self, ms: f64) {
        self.pre_delay_ms = ms;
        let samples = (self.fs * ms / 1000.0) as usize;
        self.delay_wl.set_size(samples);
        self.delay_wr.set_size(samples);
    }

    pub fn pre_delay(&self) -> f64 {
        self.pre_delay_ms
    }

    /// Makes room for a predelay of up to `max_ms` at sample rates up to `max_fs`.
    pub fn reserve_pre_delay(&mut self, max_ms: f64, max_fs: f64) {
        let n = (max_fs * max_ms / 1000.0) as usize + 2;
        self.delay_wl.reserve(n);
        self.delay_wr.reserve(n);
    }

    pub fn mute(&mut self) {
        self.delay_wl.mute();
        self.delay_wr.mute();
    }

    /// A frequency kept inside 0 … fs/2.
    pub fn lim_fs2(&self, fq: f64) -> f64 {
        fq.clamp(0.0, self.fs / 2.0)
    }
}

impl Default for Base {
    fn default() -> Self {
        Self::new()
    }
}
