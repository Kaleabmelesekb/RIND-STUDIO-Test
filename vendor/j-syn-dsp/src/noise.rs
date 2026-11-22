use std::f32::consts::TAU;

use crate::oscillator::xorshift32_next;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    White = 0,
    Pink = 1,
    Brown = 2,
}

impl NoiseType {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Pink,
            2 => Self::Brown,
            _ => Self::White,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NoiseParams {
    pub enabled: bool,
    pub noise_type: NoiseType,
    pub color: f32,  // -1.0 to +1.0 (-100% dark to +100% bright)
    pub stereo: f32, // 0.0 (mono) to 1.0 (decorrelated L/R)
    pub level: f32,  // 0.0 to 1.0 (default 0.5)
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            enabled: false,
            noise_type: NoiseType::White,
            color: 0.0,
            stereo: 0.5,
            level: 0.5,
        }
    }
}

/// Per-voice stereo White / Pink / Brown noise generator with color filter and stereo decorrelation.
#[derive(Debug, Clone, Copy)]
pub struct NoiseState {
    pub rng_l: u32,
    pub rng_r: u32,
    pub pink_b: [[f32; 7]; 2],
    pub brown_z: [f32; 2],
    pub lp_z: [f32; 2],
    pub hp_z: [f32; 2],
}

impl Default for NoiseState {
    fn default() -> Self {
        Self::new()
    }
}

impl NoiseState {
    pub fn new() -> Self {
        Self {
            rng_l: 0x1234_5679,
            rng_r: 0x8765_4321,
            pink_b: [[0.0; 7]; 2],
            brown_z: [0.0; 2],
            lp_z: [0.0; 2],
            hp_z: [0.0; 2],
        }
    }

    pub fn reset(&mut self) {
        self.pink_b = [[0.0; 7]; 2];
        self.brown_z = [0.0; 2];
        self.lp_z = [0.0; 2];
        self.hp_z = [0.0; 2];
    }

    pub fn on_note_on(&mut self, seed_rng: &mut u32) {
        self.reset();
        let _ = xorshift32_next(seed_rng);
        self.rng_l = (*seed_rng).max(1);
        let _ = xorshift32_next(seed_rng);
        let r = *seed_rng ^ 0xA5A5_5A5A;
        self.rng_r = if r == 0 { 0x5A5A_A5A5 } else { r };
    }

    #[inline]
    fn process_channel(
        &mut self,
        ch: usize,
        white: f32,
        noise_type: NoiseType,
        color: f32,
        sample_rate: f32,
    ) -> f32 {
        let base = match noise_type {
            NoiseType::White => white * 0.65,
            NoiseType::Pink => {
                // Paul Kellet's 7-pole pinking filter (-3 dB/oct)
                let b = &mut self.pink_b[ch];
                b[0] = 0.998_86 * b[0] + white * 0.055_517_9;
                b[1] = 0.993_32 * b[1] + white * 0.075_075_9;
                b[2] = 0.969_00 * b[2] + white * 0.153_852_0;
                b[3] = 0.866_50 * b[3] + white * 0.310_485_6;
                b[4] = 0.550_00 * b[4] + white * 0.532_952_2;
                b[5] = -0.761_6 * b[5] - white * 0.016_898_0;
                let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.536_2;
                b[6] = white * 0.115_926;
                pink * 0.18
            }
            NoiseType::Brown => {
                // Leaky integrator (-4.5 to -6 dB/oct)
                let z = (self.brown_z[ch] + white * 0.05).clamp(-1.0, 1.0) * 0.992;
                self.brown_z[ch] = z;
                z * 2.2
            }
        };

        let sr = sample_rate.max(1000.0);
        if color < -1e-4 {
            let fc = 200.0 * (80.0_f32).powf(1.0 + color);
            let a = (1.0 - (-TAU * fc / sr).exp()).clamp(0.001, 1.0);
            self.lp_z[ch] += (base - self.lp_z[ch]) * a;
            self.hp_z[ch] += (base - self.hp_z[ch]) * 0.005;
            self.lp_z[ch]
        } else if color > 1e-4 {
            let fc = 40.0 * (120.0_f32).powf(color);
            let a = (1.0 - (-TAU * fc / sr).exp()).clamp(0.001, 1.0);
            self.lp_z[ch] += (base - self.lp_z[ch]) * 0.5;
            self.hp_z[ch] += (base - self.hp_z[ch]) * a;
            (base - self.hp_z[ch]) * (1.0 + 0.5 * color)
        } else {
            self.lp_z[ch] += (base - self.lp_z[ch]) * 0.5;
            self.hp_z[ch] += (base - self.hp_z[ch]) * 0.005;
            base
        }
    }

    #[inline]
    pub fn process_sample(&mut self, params: &NoiseParams, sample_rate: f32) -> (f32, f32) {
        if !params.enabled || params.level <= 1e-6 {
            return (0.0, 0.0);
        }

        let w_l = xorshift32_next(&mut self.rng_l) * 2.0 - 1.0;
        let w_r = xorshift32_next(&mut self.rng_r) * 2.0 - 1.0;
        let color = params.color.clamp(-1.0, 1.0);

        let out_l = self.process_channel(0, w_l, params.noise_type, color, sample_rate);
        let out_r = self.process_channel(1, w_r, params.noise_type, color, sample_rate);

        let st = params.stereo.clamp(0.0, 1.0);
        let norm = ((1.0 - st).powi(2) + st.powi(2)).sqrt().max(0.707);
        let r_mixed = (out_l * (1.0 - st) + out_r * st) / norm;

        let level = params.level.clamp(0.0, 1.0);
        (out_l * level, r_mixed * level)
    }
}
