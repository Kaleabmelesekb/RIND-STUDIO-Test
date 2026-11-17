//! Fractal pink noise (Freeverb3's `noisegen_pink_frac`, after the TAP plugins): a midpoint-displacement pattern of
//! 2^15 samples, built half by half while the other half plays, so no block of work lands in one audio period.

const LENGTH_BITS: u32 = 15;
const LENGTH: usize = 1 << LENGTH_BITS;
const HURST: f32 = 0.5;

#[derive(Clone)]
pub struct PinkNoise {
    slot: Vec<f32>,
    count: usize,
    front: usize,
    gen_half: usize,
    gl: usize,
    gc: usize,
    gr: f32,
    seed: u32,
}

impl Default for PinkNoise {
    fn default() -> Self {
        let mut n = Self {
            slot: vec![0.0; LENGTH * 2],
            count: 0,
            front: 0,
            gen_half: 0,
            gl: LENGTH,
            gc: 0,
            gr: 0.0,
            seed: 0x1234_5678,
        };
        n.mute();
        n
    }
}

impl PinkNoise {
    #[inline]
    fn rand(&mut self) -> f32 {
        // xorshift32 to [0, 1)
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / 16_777_216.0
    }

    fn begin(&mut self, half: usize) {
        self.gen_half = half;
        self.gl = LENGTH;
        self.gr = 2.0 * (HURST * HURST) + 0.3;
        self.gc = 0;
        self.slot[half * LENGTH] = 0.0;
    }

    /// `steps` displacements; true when the half is complete.
    fn advance(&mut self, mut steps: usize) -> bool {
        let base = self.gen_half * LENGTH;
        while steps > 0 {
            if self.gl <= 1 {
                return true;
            }
            let k = LENGTH / self.gl;
            if self.gc >= k {
                self.gl /= 2;
                self.gr /= 2f32.powf(HURST);
                self.gc = 0;
                continue;
            }
            let i = self.gc * self.gl + self.gl / 2;
            let a = self.slot[base + self.gc * self.gl];
            let b = self.slot[base + ((self.gc + 1) * self.gl) % LENGTH];
            let v = (a + b) / 2.0 + 2.0 * self.gr * (self.rand() - 0.5);
            self.slot[base + i] = v.clamp(-1.0, 1.0);
            self.gc += 1;
            steps -= 1;
        }
        self.gl <= 1
    }

    /// Starts over with a full first block (not for the audio thread's every sample, but cheap enough on a reset).
    pub fn mute(&mut self) {
        self.slot.fill(0.0);
        self.count = 0;
        self.front = 0;
        self.begin(0);
        while !self.advance(LENGTH) {}
        self.count = LENGTH;
        self.begin(1);
    }

    #[inline]
    pub fn process(&mut self) -> f32 {
        if self.count == 0 {
            while !self.advance(LENGTH) {}
            self.front ^= 1;
            self.count = LENGTH;
            let spare = self.front ^ 1;
            self.begin(spare);
        }
        let out = self.slot[self.front * LENGTH + LENGTH - self.count];
        self.count -= 1;
        self.advance(2);
        out
    }
}
