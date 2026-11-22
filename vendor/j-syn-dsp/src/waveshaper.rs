//! Shared zero-allocation waveshaping transfer curves for `DistortionStereo` and `InsertShaperState`.

use std::f32::consts::FRAC_PI_2;

use crate::filter::fast_tanh;

/// Master Distortion FX & shared waveshaper algorithm selector.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistortionMode {
    #[default]
    Tube = 0,
    Mech = 1,
    Sine = 2,
    Crush = 3,
    ClassicA = 4,
    ClassicB = 5,
    ClassicC = 6,
    DiodeA = 7,
    DiodeB = 8,
    LinearFold = 9,
    SineFold = 10,
    HardClip = 11,
    HalfRectify = 12,
    FullRectify = 13,
}

impl DistortionMode {
    pub const ALL: [DistortionMode; 14] = [
        Self::Tube,
        Self::Mech,
        Self::Sine,
        Self::Crush,
        Self::ClassicA,
        Self::ClassicB,
        Self::ClassicC,
        Self::DiodeA,
        Self::DiodeB,
        Self::LinearFold,
        Self::SineFold,
        Self::HardClip,
        Self::HalfRectify,
        Self::FullRectify,
    ];

    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Tube,
            1 => Self::Mech,
            2 => Self::Sine,
            3 => Self::Crush,
            4 => Self::ClassicA,
            5 => Self::ClassicB,
            6 => Self::ClassicC,
            7 => Self::DiodeA,
            8 => Self::DiodeB,
            9 => Self::LinearFold,
            10 => Self::SineFold,
            11 => Self::HardClip,
            12 => Self::HalfRectify,
            13 => Self::FullRectify,
            _ => Self::Tube,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tube => "Tube",
            Self::Mech => "Mech",
            Self::Sine => "Sine",
            Self::Crush => "Crush",
            Self::ClassicA => "Classic A",
            Self::ClassicB => "Classic B",
            Self::ClassicC => "Classic C",
            Self::DiodeA => "Diode A",
            Self::DiodeB => "Diode B",
            Self::LinearFold => "Linear Fold",
            Self::SineFold => "Sine Fold",
            Self::HardClip => "Hard Clip",
            Self::HalfRectify => "Half Rect",
            Self::FullRectify => "Full Rect",
        }
    }
}

#[inline]
pub fn tube_saturate(x: f32, drive: f32) -> f32 {
    let gain = 1.0 + drive * 19.0;
    let bias = drive * 0.08;
    (x * gain + bias).tanh() - bias.tanh()
}

#[inline]
pub fn mech_clip(x: f32, drive: f32) -> f32 {
    let gain = 1.0 + drive * 29.0;
    let s = x * gain;
    if s > 1.0 {
        1.0
    } else if s < -1.0 {
        -1.0
    } else {
        let k = 0.15 * drive;
        s - k * (s * s * s)
    }
}

#[inline]
pub fn shape_sine(x: f32, drive: f32) -> f32 {
    let gain = 1.0 + drive * 6.0;
    (x * gain).sin() * 0.85
}

#[inline]
pub fn shape_parabolic(x: f32, drive: f32) -> f32 {
    let g = 1.0 + drive * 4.0;
    let u = x * g;
    let asym = 0.25 * drive * u * u;
    fast_tanh(u + asym) - 0.15 * fast_tanh(asym)
}

#[inline]
pub fn shape_soft_clip(x: f32, drive: f32) -> f32 {
    let g = 1.0 + drive * 8.0;
    fast_tanh(x * g)
}

#[inline]
fn classic_a_core(u: f32, tb: f32) -> f32 {
    ((u + tb).tanh() - tb.tanh()) * 0.92
}

#[inline]
fn classic_b_core(u: f32, tb: f32) -> f32 {
    ((u + tb).tanh() - tb.tanh()) * 0.85
}

#[inline]
fn classic_c_core(u: f32, fold: f32) -> f32 {
    let norm = 1.0 / (1.0 + 0.45 * fold);
    (u.tanh() + fold * u * (-0.5 * u * u).exp()) * norm
}

/// Germanium-style diode clipper: soft, early-knee asymmetric rational curve.
#[inline]
fn diode_a_core(u: f32) -> f32 {
    if u >= 0.0 {
        (0.72 * u) / (0.32 + u)
    } else {
        (0.88 * u) / (0.42 - u)
    }
}

/// Silicon-style diode clipper: higher threshold, sharper asymmetric knee.
#[inline]
fn diode_b_core(u: f32) -> f32 {
    let kp = 0.58;
    let kn = 0.72;
    if u > kp {
        kp + (1.0 - kp) * ((u - kp) / (1.0 - kp)).tanh()
    } else if u < -kn {
        -kn + (1.0 - kn) * ((u + kn) / (1.0 - kn)).tanh()
    } else {
        u
    }
}

/// Triangle mirror reflection fold (`[-1.0, 1.0]` identity, reflects smoothly beyond).
#[inline]
pub fn linear_fold_core(u: f32) -> f32 {
    let p = ((u + 1.0) * 0.25).rem_euclid(1.0);
    1.0 - 4.0 * (p - 0.5).abs()
}

/// Evaluate a continuous waveshaping algorithm on a single sample.
///
/// - `drive`: `0.0..=1.0`
/// - `bias`: `-1.0..=1.0` (DC-compensated asymmetry pre-shaper; `0.0` = neutral)
///
/// Note: `DistortionMode::Crush` is stateful (sample-and-hold) and falls back to
/// bit-quantization without decimation when evaluated sample-by-sample (e.g. scope preview).
#[inline]
pub fn shape_sample(mode: DistortionMode, x: f32, drive: f32, bias: f32) -> f32 {
    let d = drive.clamp(0.0, 1.0);
    let b = bias.clamp(-1.0, 1.0);

    let y = match mode {
        DistortionMode::Tube => {
            let g = 1.0 + d * 19.0;
            let tb = d * 0.08 + b * 0.45;
            (x * g + tb).tanh() - tb.tanh()
        }
        DistortionMode::Mech => {
            let bo = b * 0.25;
            mech_clip(x + bo, d) - mech_clip(bo, d)
        }
        DistortionMode::Sine => {
            let g = 1.0 + d * 6.0;
            let bo = b * 0.35;
            ((x + bo) * g).sin() * 0.85 - (bo * g).sin() * 0.85
        }
        DistortionMode::Crush => {
            let levels = 2.0_f32.powf(11.0 - d * 8.5);
            let bo = b * 0.2;
            (((x + bo).clamp(-1.5, 1.5) * levels).round() / levels)
                - ((bo.clamp(-1.5, 1.5) * levels).round() / levels)
        }
        DistortionMode::ClassicA => {
            let g = 1.0 + d * 14.0;
            let tb = 0.14 * d * (1.0 - 0.65 * d) + b * 0.45;
            classic_a_core(x * g, tb)
        }
        DistortionMode::ClassicB => {
            let g = 1.6 + d * 14.0;
            let tb = 0.22 + 0.16 * d + b * 0.45;
            classic_b_core(x * g, tb)
        }
        DistortionMode::ClassicC => {
            let g = 1.0 + d * 16.0;
            let fold = 0.95 * ((d - 0.18).max(0.0) / 0.82);
            let bo = b * 0.45;
            classic_c_core(x * g + bo, fold) - classic_c_core(bo, fold)
        }
        DistortionMode::DiodeA => {
            let g = 1.5 + d * 16.0;
            let bo = b * 0.45;
            diode_a_core(x * g + bo) - diode_a_core(bo)
        }
        DistortionMode::DiodeB => {
            let g = 1.2 + d * 18.0;
            let bo = b * 0.45;
            diode_b_core(x * g + bo) - diode_b_core(bo)
        }
        DistortionMode::LinearFold => {
            let g = 1.0 + d * 7.0;
            let bo = b * 0.35;
            linear_fold_core(x * g + bo) - linear_fold_core(bo)
        }
        DistortionMode::SineFold => {
            let g = (1.0 + d * 6.0) * FRAC_PI_2;
            let bo = b * 0.25;
            ((x + bo) * g).sin() - (bo * g).sin()
        }
        DistortionMode::HardClip => {
            let g = 1.0 + d * 14.0;
            let bo = b * 0.35;
            (x * g + bo).clamp(-1.0, 1.0) - bo.clamp(-1.0, 1.0)
        }
        DistortionMode::HalfRectify => {
            let g = 1.0 + d * 6.0;
            let bo = b * 0.35;
            let eval = |u: f32| {
                if u >= 0.0 {
                    u.tanh()
                } else {
                    (u * (1.0 - d) * 0.35).tanh()
                }
            };
            eval(x * g + bo) - eval(bo)
        }
        DistortionMode::FullRectify => {
            let g = 1.0 + d * 6.0;
            let bo = b * 0.35;
            let eval = |u: f32| (u.abs() * d + u * (1.0 - d)).tanh();
            eval(x * g + bo) - eval(bo)
        }
    };

    if y.is_finite() {
        y
    } else {
        0.0
    }
}

/// Maps legacy Distortion `p[0]` quartiles (`<0.25 -> Tube`, `<0.50 -> Mech`, `<0.75 -> Sine`,
/// `>=0.75 -> Crush`) to `DistortionMode` indices `0..=3`.
#[inline]
pub fn legacy_distortion_submode_index(p0: f32) -> usize {
    if p0 < 0.25 {
        0
    } else if p0 < 0.50 {
        1
    } else if p0 < 0.75 {
        2
    } else {
        3
    }
}
