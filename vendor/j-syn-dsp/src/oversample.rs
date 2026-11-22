//! Zero-allocation 4x polyphase IIR half-band oversampler and RBJ shelf biquads.
//! Ported from `crushclone-rs` (`Oversampler4x`, `Biquad`, `BiquadCoeffs`).

use std::f64::consts::PI;

const STAGE1_A0: [f64; 6] = [
    0.00783966311367806,
    0.06963352550176428,
    0.18810432330303528,
    0.3532239775914464,
    0.5540947331223993,
    0.7891211017044416,
];

const STAGE1_A1: [f64; 6] = [
    0.03120668850021657,
    0.12231433460067268,
    0.2655839409913821,
    0.44966486124254507,
    0.6666935772892053,
    0.925071404637986,
];

const STAGE2_A0: [f64; 3] = [
    0.01582607515569411,
    0.15195740895758536,
    0.48910206447339805,
];

const STAGE2_A1: [f64; 3] = [
    0.06481785456179254,
    0.28723593392249314,
    0.7918974147193385,
];

#[derive(Debug, Clone, Copy)]
struct AllpassCascade<const N: usize> {
    coeffs: [f64; N],
    x_prev: [f64; N],
    y_prev: [f64; N],
}

impl<const N: usize> AllpassCascade<N> {
    const fn new(coeffs: [f64; N]) -> Self {
        Self {
            coeffs,
            x_prev: [0.0; N],
            y_prev: [0.0; N],
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.x_prev = [0.0; N];
        self.y_prev = [0.0; N];
    }

    #[inline]
    fn process(&mut self, mut x: f64) -> f64 {
        for i in 0..N {
            let y = self.coeffs[i] * (x - self.y_prev[i]) + self.x_prev[i];
            self.x_prev[i] = x;
            self.y_prev[i] = y;
            x = y;
        }
        x
    }
}

#[derive(Debug, Clone, Copy)]
struct HalfBand2x<const N: usize> {
    up_a0: AllpassCascade<N>,
    up_a1: AllpassCascade<N>,
    dn_a0: AllpassCascade<N>,
    dn_a1: AllpassCascade<N>,
}

impl<const N: usize> HalfBand2x<N> {
    const fn new(a0: [f64; N], a1: [f64; N]) -> Self {
        Self {
            up_a0: AllpassCascade::new(a0),
            up_a1: AllpassCascade::new(a1),
            dn_a0: AllpassCascade::new(a0),
            dn_a1: AllpassCascade::new(a1),
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.up_a0.reset();
        self.up_a1.reset();
        self.dn_a0.reset();
        self.dn_a1.reset();
    }

    #[inline]
    fn upsample(&mut self, x: f64) -> [f64; 2] {
        let s0 = self.up_a0.process(x);
        let s1 = self.up_a1.process(x);
        [s0, s1]
    }

    #[inline]
    fn downsample(&mut self, s0: f64, s1: f64) -> f64 {
        0.5 * (self.dn_a0.process(s1) + self.dn_a1.process(s0))
    }
}

/// Mono 4x polyphase IIR half-band oversampler with zero heap allocations.
#[derive(Debug, Clone, Copy)]
pub struct Oversampler4x {
    stage1: HalfBand2x<6>,
    stage2: HalfBand2x<3>,
}

impl Default for Oversampler4x {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler4x {
    pub const fn new() -> Self {
        Self {
            stage1: HalfBand2x::new(STAGE1_A0, STAGE1_A1),
            stage2: HalfBand2x::new(STAGE2_A0, STAGE2_A1),
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.stage1.reset();
        self.stage2.reset();
    }

    /// Upsamples one 1x sample into 4 samples at 4x rate, applies `f` to each
    /// 4x sample in chronological order, and downsamples back to 1x.
    #[inline]
    pub fn process_sample<F>(&mut self, input: f32, mut f: F) -> f32
    where
        F: FnMut(f32) -> f32,
    {
        let [x2_0, x2_1] = self.stage1.upsample(input as f64);
        let [x4_0, x4_1] = self.stage2.upsample(x2_0);
        let [x4_2, x4_3] = self.stage2.upsample(x2_1);

        let y4_0 = f(x4_0 as f32) as f64;
        let y4_1 = f(x4_1 as f32) as f64;
        let y4_2 = f(x4_2 as f32) as f64;
        let y4_3 = f(x4_3 as f32) as f64;

        let y2_0 = self.stage2.downsample(y4_0, y4_1);
        let y2_1 = self.stage2.downsample(y4_2, y4_3);

        self.stage1.downsample(y2_0, y2_1) as f32
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

impl BiquadCoeffs {
    #[inline]
    fn normalize(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    #[inline]
    fn safe_w0(sample_rate: f64, freq_hz: f64) -> f64 {
        let clamped_freq = freq_hz.clamp(1.0, sample_rate * 0.45);
        2.0 * PI * clamped_freq / sample_rate
    }

    /// 2nd-order RBJ Low Shelf filter with linear gain factor.
    pub fn low_shelf_linear(sample_rate: f64, freq_hz: f64, q: f64, linear_gain: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = linear_gain.max(1e-6).sqrt();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalize(b0, b1, b2, a0, a1, a2)
    }

    /// 2nd-order RBJ High Shelf filter with linear gain factor.
    pub fn high_shelf_linear(sample_rate: f64, freq_hz: f64, q: f64, linear_gain: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = linear_gain.max(1e-6).sqrt();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalize(b0, b1, b2, a0, a1, a2)
    }

    /// 2nd-order RBJ Low-Pass biquad.
    #[inline]
    pub fn low_pass(sample_rate: f64, freq_hz: f64, q: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        let b1 = 1.0 - cos_w0;
        let b0 = 0.5 * b1;
        Self::normalize(b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
    }

    /// 2nd-order RBJ High-Pass biquad.
    #[inline]
    pub fn high_pass(sample_rate: f64, freq_hz: f64, q: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        let b0 = 0.5 * (1.0 + cos_w0);
        let b1 = -(1.0 + cos_w0);
        Self::normalize(b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
    }

    /// 2nd-order RBJ All-Pass biquad (unity magnitude, steep phase rotation at `freq_hz`).
    #[inline]
    pub fn all_pass(sample_rate: f64, freq_hz: f64, q: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        Self::normalize(
            1.0 - alpha,
            -2.0 * cos_w0,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }
}

/// Direct Form II Transposed biquad filter (64-bit internal state for precision).
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    coeffs: BiquadCoeffs,
    z1: f64,
    z2: f64,
}

impl Biquad {
    #[inline]
    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let x = input as f64;
        let y = self.coeffs.b0 * x + self.z1;
        self.z1 = self.coeffs.b1 * x - self.coeffs.a1 * y + self.z2;
        self.z2 = self.coeffs.b2 * x - self.coeffs.a2 * y;
        y as f32
    }
}
