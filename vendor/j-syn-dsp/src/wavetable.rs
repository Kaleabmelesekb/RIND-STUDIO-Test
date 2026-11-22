use std::f32::consts::{PI, TAU};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwap;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub const FRAME_LEN: usize = 2048;
pub const MAX_FRAMES: usize = 256;
/// Mip levels 0..11: max harmonics = 1024 >> level (1024 down to 1).
pub const NUM_MIP_LEVELS: usize = 11;
pub const NUM_FACTORY_WAVETABLES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactoryWavetable {
    BasicShapes = 0,
    DigitalFormant = 1,
    HarmonicSync = 2,
    AggressivePwm = 3,
}

impl FactoryWavetable {
    pub const ALL: [Self; 4] = [
        Self::BasicShapes,
        Self::DigitalFormant,
        Self::HarmonicSync,
        Self::AggressivePwm,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BasicShapes => "Basic Shapes",
            Self::DigitalFormant => "Digital Formant",
            Self::HarmonicSync => "Harmonic Sync",
            Self::AggressivePwm => "Aggressive PWM",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::BasicShapes,
            1 => Self::DigitalFormant,
            2 => Self::HarmonicSync,
            _ => Self::AggressivePwm,
        }
    }
}

/// A 2048-sample/frame wavetable with pre-computed FFT band-limited mip-map octaves.
pub struct Wavetable {
    pub name: String,
    pub num_frames: usize,
    pub raw_frames: Vec<[f32; FRAME_LEN]>,
    /// `mip_maps[mip_level][frame_idx]`
    pub mip_maps: Vec<Vec<[f32; FRAME_LEN]>>,
}

impl Wavetable {
    /// Builds a wavetable from 2048-sample frames and pre-computes FFT band-limited mip-map octaves.
    pub fn from_frames(name: impl Into<String>, mut frames: Vec<[f32; FRAME_LEN]>) -> Self {
        if frames.is_empty() {
            let mut sine = [0.0_f32; FRAME_LEN];
            for (i, s) in sine.iter_mut().enumerate() {
                *s = (TAU * i as f32 / FRAME_LEN as f32).sin();
            }
            frames.push(sine);
        }
        if frames.len() > MAX_FRAMES {
            frames.truncate(MAX_FRAMES);
        }

        let num_frames = frames.len();
        let mut planner = FftPlanner::<f32>::new();
        let fft_fwd = planner.plan_fft_forward(FRAME_LEN);
        let fft_inv = planner.plan_fft_inverse(FRAME_LEN);

        let mut mip_maps = vec![vec![[0.0_f32; FRAME_LEN]; num_frames]; NUM_MIP_LEVELS];
        let mut spectrum = vec![Complex::new(0.0_f32, 0.0_f32); FRAME_LEN];
        let mut work = vec![Complex::new(0.0_f32, 0.0_f32); FRAME_LEN];

        let inv_n = 1.0 / FRAME_LEN as f32;

        for (f_idx, frame) in frames.iter().enumerate() {
            for (i, &sample) in frame.iter().enumerate() {
                spectrum[i] = Complex::new(sample, 0.0);
            }
            fft_fwd.process(&mut spectrum);

            for (mip_idx, mip_level) in mip_maps.iter_mut().enumerate() {
                let max_harm = ((FRAME_LEN / 2) >> mip_idx).max(1);
                work.fill(Complex::new(0.0, 0.0));
                // Remove DC (bin 0) and keep harmonics 1..=max_harm with conjugate symmetry.
                for h in 1..=max_harm {
                    if h < FRAME_LEN / 2 {
                        work[h] = spectrum[h];
                        work[FRAME_LEN - h] = spectrum[FRAME_LEN - h];
                    } else if h == FRAME_LEN / 2 {
                        work[h] = spectrum[h];
                    }
                }

                fft_inv.process(&mut work);
                let out_frame = &mut mip_level[f_idx];
                for (i, c) in work.iter().enumerate() {
                    out_frame[i] = c.re * inv_n;
                }
            }
        }

        Self {
            name: name.into(),
            num_frames,
            raw_frames: frames,
            mip_maps,
        }
    }

    /// Loads a 2048-sample/frame single-cycle or multi-frame `.wav` wavetable file.
    pub fn load_from_wav(path: &Path) -> Result<Self, String> {
        let mut reader = hound::WavReader::open(path)
            .map_err(|e| format!("Failed to open WAV '{}': {e}", path.display()))?;
        let spec = reader.spec();
        let channels = spec.channels.max(1) as usize;

        let raw_samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
            (hound::SampleFormat::Float, 32) => reader
                .samples::<f32>()
                .map(|s| s.unwrap_or(0.0))
                .collect(),
            (hound::SampleFormat::Int, 8) => reader
                .samples::<i8>()
                .map(|s| s.unwrap_or(0) as f32 / 128.0)
                .collect(),
            (hound::SampleFormat::Int, 16) => reader
                .samples::<i16>()
                .map(|s| s.unwrap_or(0) as f32 / 32768.0)
                .collect(),
            (hound::SampleFormat::Int, 24) => reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / 8_388_608.0)
                .collect(),
            (hound::SampleFormat::Int, 32) => reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / 2_147_483_648.0)
                .collect(),
            _ => {
                return Err(format!(
                    "Unsupported WAV format: {:?} {}-bit",
                    spec.sample_format, spec.bits_per_sample
                ));
            }
        };

        if raw_samples.is_empty() {
            return Err("WAV file contains no audio samples".to_string());
        }

        // Mix down to mono.
        let mono: Vec<f32> = if channels == 1 {
            raw_samples
        } else {
            let inv_ch = 1.0 / channels as f32;
            raw_samples
                .chunks_exact(channels)
                .map(|ch| ch.iter().sum::<f32>() * inv_ch)
                .collect()
        };

        if mono.is_empty() {
            return Err("WAV file contains no complete mono frames".to_string());
        }

        let mut frames = Vec::new();
        if mono.len() < FRAME_LEN {
            // Single-cycle shorter than 2048 samples: linearly resample to 2048.
            let mut frame = [0.0_f32; FRAME_LEN];
            let src_len = mono.len();
            for (i, out) in frame.iter_mut().enumerate() {
                let pos = (i as f32 / FRAME_LEN as f32) * src_len as f32;
                let idx0 = (pos as usize) % src_len;
                let idx1 = (idx0 + 1) % src_len;
                let frac = pos - pos.floor();
                *out = mono[idx0] + (mono[idx1] - mono[idx0]) * frac;
            }
            frames.push(frame);
        } else {
            for chunk in mono.chunks(FRAME_LEN).take(MAX_FRAMES) {
                if chunk.len() == FRAME_LEN {
                    let mut frame = [0.0_f32; FRAME_LEN];
                    frame.copy_from_slice(chunk);
                    frames.push(frame);
                } else if frames.is_empty() {
                    let mut frame = [0.0_f32; FRAME_LEN];
                    frame[..chunk.len()].copy_from_slice(chunk);
                    frames.push(frame);
                }
            }
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom WAV")
            .to_string();

        Ok(Self::from_frames(name, frames))
    }

    /// Vintage 2007 mode: single-table cubic Hermite interpolation with zero oversampling.
    #[inline]
    pub fn sample_2007(&self, wt_pos: f32, phase: f32) -> f32 {
        let p = phase.rem_euclid(1.0);
        let pos = wt_pos.clamp(0.0, 1.0) * (self.num_frames - 1) as f32;
        let f0 = (pos as usize).min(self.num_frames - 1);
        let f1 = (f0 + 1).min(self.num_frames - 1);
        let ff = pos - f0 as f32;

        let s0 = hermite_sample(&self.raw_frames[f0], p);
        if f0 == f1 || ff <= 1e-6 {
            s0
        } else {
            let s1 = hermite_sample(&self.raw_frames[f1], p);
            s0 + (s1 - s0) * ff
        }
    }

    /// HQ mode: pre-computed FFT band-limited mip-map octaves with trilinear interpolation
    /// (`table_pos` x `phase` x `mip_level`).
    #[inline]
    pub fn sample_hq(&self, wt_pos: f32, phase: f32, phase_inc: f32) -> f32 {
        let p = phase.rem_euclid(1.0);
        let max_harm = (0.5 / phase_inc.abs().max(1e-6)).max(1.0);
        let mip_f = ((FRAME_LEN as f32 * 0.5) / max_harm)
            .max(1.0)
            .log2()
            .clamp(0.0, (NUM_MIP_LEVELS - 1) as f32);

        let m0 = (mip_f as usize).min(NUM_MIP_LEVELS - 1);
        let m1 = (m0 + 1).min(NUM_MIP_LEVELS - 1);
        let mf = mip_f - m0 as f32;

        let pos = wt_pos.clamp(0.0, 1.0) * (self.num_frames - 1) as f32;
        let f0 = (pos as usize).min(self.num_frames - 1);
        let f1 = (f0 + 1).min(self.num_frames - 1);
        let ff = pos - f0 as f32;

        let a = linear_sample(&self.mip_maps[m0][f0], p);
        let b = if f0 == f1 {
            a
        } else {
            linear_sample(&self.mip_maps[m0][f1], p)
        };
        let s_m0 = a + (b - a) * ff;

        if m0 == m1 || mf <= 1e-6 {
            return s_m0;
        }

        let c = linear_sample(&self.mip_maps[m1][f0], p);
        let d = if f0 == f1 {
            c
        } else {
            linear_sample(&self.mip_maps[m1][f1], p)
        };
        let s_m1 = c + (d - c) * ff;

        s_m0 + (s_m1 - s_m0) * mf
    }

    /// Fills `out` with an interpolated waveform preview at `wt_pos` for the GUI canvas.
    pub fn fill_preview(&self, wt_pos: f32, out: &mut [f32]) {
        if out.is_empty() {
            return;
        }
        let pos = wt_pos.clamp(0.0, 1.0) * (self.num_frames - 1) as f32;
        let f0 = (pos as usize).min(self.num_frames - 1);
        let f1 = (f0 + 1).min(self.num_frames - 1);
        let ff = pos - f0 as f32;

        let len = out.len();
        for (i, dst) in out.iter_mut().enumerate() {
            let idx = (i * FRAME_LEN) / len;
            let a = self.raw_frames[f0][idx];
            let b = self.raw_frames[f1][idx];
            *dst = a + (b - a) * ff;
        }
    }
}

#[inline]
fn linear_sample(frame: &[f32; FRAME_LEN], phase: f32) -> f32 {
    let f_idx = phase * FRAME_LEN as f32;
    let i0 = (f_idx as usize) & (FRAME_LEN - 1);
    let i1 = (i0 + 1) & (FRAME_LEN - 1);
    let frac = f_idx - f_idx.floor();
    frame[i0] + (frame[i1] - frame[i0]) * frac
}

#[inline]
fn hermite_sample(frame: &[f32; FRAME_LEN], phase: f32) -> f32 {
    let f_idx = phase * FRAME_LEN as f32;
    let i1 = (f_idx as usize) & (FRAME_LEN - 1);
    let i0 = (i1 + FRAME_LEN - 1) & (FRAME_LEN - 1);
    let i2 = (i1 + 1) & (FRAME_LEN - 1);
    let i3 = (i1 + 2) & (FRAME_LEN - 1);
    let t = f_idx - f_idx.floor();

    let y0 = frame[i0];
    let y1 = frame[i1];
    let y2 = frame[i2];
    let y3 = frame[i3];

    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);

    ((c3 * t + c2) * t + c1) * t + c0
}

/// Generates (or returns cached) the 4 built-in factory wavetables.
pub fn build_factory_wavetables() -> [Arc<Wavetable>; NUM_FACTORY_WAVETABLES] {
    static FACTORY_TABLES: OnceLock<[Arc<Wavetable>; NUM_FACTORY_WAVETABLES]> = OnceLock::new();
    FACTORY_TABLES
        .get_or_init(|| {
            [
                Arc::new(build_basic_shapes()),
                Arc::new(build_digital_formant()),
                Arc::new(build_harmonic_sync()),
                Arc::new(build_aggressive_pwm()),
            ]
        })
        .clone()
}

fn build_basic_shapes() -> Wavetable {
    const N_FRAMES: usize = 64;
    let mut frames = Vec::with_capacity(N_FRAMES);

    for f in 0..N_FRAMES {
        let t = f as f32 / (N_FRAMES - 1) as f32;
        let mut frame = [0.0_f32; FRAME_LEN];
        for (i, sample) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_LEN as f32;
            let sine = (TAU * p).sin();
            let tri = 1.0 - 4.0 * (p - 0.5).abs();
            let saw = 2.0 * p - 1.0;
            let sqr = if p < 0.5 { 1.0 } else { -1.0 };

            *sample = if t < 1.0 / 3.0 {
                let u = t * 3.0;
                sine + (tri - sine) * u
            } else if t < 2.0 / 3.0 {
                let u = (t - 1.0 / 3.0) * 3.0;
                tri + (saw - tri) * u
            } else {
                let u = (t - 2.0 / 3.0) * 3.0;
                saw + (sqr - saw) * u
            };
        }
        frames.push(frame);
    }

    Wavetable::from_frames(FactoryWavetable::BasicShapes.name(), frames)
}

fn build_digital_formant() -> Wavetable {
    const N_FRAMES: usize = 64;
    let mut frames = Vec::with_capacity(N_FRAMES);

    for f in 0..N_FRAMES {
        let t = f as f32 / (N_FRAMES - 1) as f32;
        let center_h = 2.0 + t * 22.0;
        let second_h = 6.0 + (1.0 - t) * 18.0;
        let mut frame = [0.0_f32; FRAME_LEN];
        let mut max_abs = 0.0_f32;

        for (i, sample) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_LEN as f32;
            let mut acc = (TAU * p).sin() * 0.65;
            for h in 2..=32 {
                let hf = h as f32;
                let g1 = (-0.5 * ((hf - center_h) / 2.2).powi(2)).exp();
                let g2 = (-0.5 * ((hf - second_h) / 3.0).powi(2)).exp() * 0.55;
                let amp = (g1 + g2) / (1.0 + 0.05 * hf);
                acc += (TAU * p * hf).sin() * amp;
            }
            *sample = acc;
            max_abs = max_abs.max(acc.abs());
        }
        if max_abs > 1e-6 {
            let norm = 0.95 / max_abs;
            for s in frame.iter_mut() {
                *s *= norm;
            }
        }
        frames.push(frame);
    }

    Wavetable::from_frames(FactoryWavetable::DigitalFormant.name(), frames)
}

fn build_harmonic_sync() -> Wavetable {
    const N_FRAMES: usize = 64;
    let mut frames = Vec::with_capacity(N_FRAMES);

    for f in 0..N_FRAMES {
        let t = f as f32 / (N_FRAMES - 1) as f32;
        let sync_ratio = 1.0 + t * 7.0;
        let mut frame = [0.0_f32; FRAME_LEN];
        for (i, sample) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_LEN as f32;
            let slave_p = (p * sync_ratio).fract();
            let saw = 2.0 * slave_p - 1.0;
            // Smooth Tukey-like edge fade near master wrap to keep fundamental coherent
            let win = (PI * p).sin().powf(0.25);
            *sample = saw * win;
        }
        frames.push(frame);
    }

    Wavetable::from_frames(FactoryWavetable::HarmonicSync.name(), frames)
}

fn build_aggressive_pwm() -> Wavetable {
    const N_FRAMES: usize = 64;
    let mut frames = Vec::with_capacity(N_FRAMES);

    for f in 0..N_FRAMES {
        let t = f as f32 / (N_FRAMES - 1) as f32;
        let duty = 0.50 - t * 0.46; // 50% down to 4%
        let grit = t * 0.28;
        let mut frame = [0.0_f32; FRAME_LEN];
        for (i, sample) in frame.iter_mut().enumerate() {
            let p = i as f32 / FRAME_LEN as f32;
            let pulse = if p < duty { 1.0 } else { -1.0 };
            let sub_fold = (TAU * p * 3.0).sin() * grit;
            *sample = (pulse + sub_fold).clamp(-1.0, 1.0);
        }
        frames.push(frame);
    }

    Wavetable::from_frames(FactoryWavetable::AggressivePwm.name(), frames)
}

/// Lock-free wavetable bank shared between GUI/loader threads and the audio thread.
pub struct WavetableBank {
    pub factory: [Arc<Wavetable>; NUM_FACTORY_WAVETABLES],
    pub osc_tables: [ArcSwap<Wavetable>; 4],
}

impl WavetableBank {
    pub fn new() -> Self {
        let factory = build_factory_wavetables();
        let osc_tables = [
            ArcSwap::from(Arc::clone(&factory[0])),
            ArcSwap::from(Arc::clone(&factory[1])),
            ArcSwap::from(Arc::clone(&factory[2])),
            ArcSwap::from(Arc::clone(&factory[3])),
        ];
        Self {
            factory,
            osc_tables,
        }
    }

    #[inline]
    pub fn factory_table(&self, preset: FactoryWavetable) -> &Wavetable {
        &self.factory[preset as usize]
    }

    pub fn select_factory(&self, osc_idx: usize, factory: FactoryWavetable) {
        if osc_idx < 4 {
            self.osc_tables[osc_idx].store(Arc::clone(&self.factory[factory as usize]));
        }
    }

    pub fn swap_custom(&self, osc_idx: usize, wavetable: Arc<Wavetable>) {
        if osc_idx < 4 {
            self.osc_tables[osc_idx].store(wavetable);
        }
    }
}

impl Default for WavetableBank {
    fn default() -> Self {
        Self::new()
    }
}
