//! Second-order Antiderivative Antialiasing (ADAA2) hyperbolic tangent waveshaper.
//!
//! Ported from Cure Audio `Scream` (`src/libs/ADAA.h` and `src/libs/polylogarithm_Li2.h`):
//! - ADAA: BSD 3-Clause License, Copyright (c) 2020, jatinchowdhury18
//! - Polylogarithm (`li2`): MIT License, Copyright (c) 2021 Alexander Voigt

const ADAA_TOL: f64 = 1.0e-5;

/// Real dilogarithm `Li_2(x)` rational approximation (max error ~5e-17).
///
/// Ported from Alexander Voigt's Polylogarithm (MIT License).
#[inline]
fn li2(x: f64) -> f64 {
    const PI: f64 = std::f64::consts::PI;
    const P: [f64; 6] = [
        0.999_999_999_999_999_9,
        -2.688_392_681_856_542_3,
        2.647_722_269_947_311,
        -1.153_855_960_788_741_6,
        0.208_860_777_950_206_08,
        -0.010_859_777_134_152_463,
    ];
    const Q: [f64; 7] = [
        1.0,
        -2.938_392_681_856_563_5,
        3.271_209_329_301_863_5,
        -1.707_670_217_395_429,
        0.415_960_172_284_006_04,
        -0.039_801_343_754_084_48,
        0.000_827_436_689_744_666_6,
    ];

    let y: f64;
    let r: f64;
    let s: f64;

    if x < -1.0 {
        let l = (1.0 - x).ln();
        y = 1.0 / (1.0 - x);
        r = -PI * PI / 6.0 + l * (0.5 * l - (-x).ln());
        s = 1.0;
    } else if x == -1.0 {
        return -PI * PI / 12.0;
    } else if x < 0.0 {
        let l = (-x).ln_1p();
        y = x / (x - 1.0);
        r = -0.5 * l * l;
        s = -1.0;
    } else if x == 0.0 {
        return 0.0;
    } else if x < 0.5 {
        y = x;
        r = 0.0;
        s = 1.0;
    } else if x < 1.0 {
        y = 1.0 - x;
        r = PI * PI / 6.0 - x.ln() * (-x).ln_1p();
        s = -1.0;
    } else if x == 1.0 {
        return PI * PI / 6.0;
    } else if x < 2.0 {
        let l = x.ln();
        y = 1.0 - 1.0 / x;
        r = PI * PI / 6.0 - l * (y.ln() + 0.5 * l);
        s = 1.0;
    } else {
        let l = x.ln();
        y = 1.0 / x;
        r = PI * PI / 3.0 - 0.5 * l * l;
        s = -1.0;
    }

    let p = P[0] + y * (P[1] + y * (P[2] + y * (P[3] + y * (P[4] + y * P[5]))));
    let q = Q[0] + y * (Q[1] + y * (Q[2] + y * (Q[3] + y * (Q[4] + y * (Q[5] + y * Q[6])))));

    r + s * y * (p / q)
}

#[inline]
fn adaa_tanh_ad1(x: f64) -> f64 {
    x.cosh().ln()
}

#[inline]
fn adaa_tanh_ad2(x: f64) -> f64 {
    let exp_val = (-2.0 * x).exp();
    0.5 * (li2(-exp_val) - x * (x + 2.0 * (exp_val + 1.0).ln() - 2.0 * x.cosh().ln()))
        + (std::f64::consts::PI * std::f64::consts::PI / 24.0)
}

/// Double-precision 2nd-order ADAA `tanh` state (zero allocations, `Copy`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TanhAdaa2 {
    x1: f64,
    x2: f64,
    d2: f64,
    ad2_x1: f64,
}

impl TanhAdaa2 {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn process(&mut self, x_raw: f64) -> f64 {
        // Clamp to [-40, 40] so `cosh(x)` and `exp(-2x)` stay finite in `f64` while `tanh(+-40) == +-1`.
        let x = x_raw.clamp(-40.0, 40.0);
        let ad2_x0 = adaa_tanh_ad2(x);

        let d1 = if (x - self.x1).abs() < ADAA_TOL {
            adaa_tanh_ad1(0.5 * (x + self.x1))
        } else {
            (ad2_x0 - self.ad2_x1) / (x - self.x1)
        };

        let y = if (x - self.x2).abs() < ADAA_TOL {
            let x_bar = 0.5 * (x + self.x2);
            let delta = x_bar - self.x1;
            if delta.abs() < ADAA_TOL {
                (0.5 * (x_bar + self.x1)).tanh()
            } else {
                let ad1 = adaa_tanh_ad1(x_bar);
                let ad2 = adaa_tanh_ad2(x_bar);
                (2.0 / delta) * (ad1 + (self.ad2_x1 - ad2) / delta)
            }
        } else {
            (2.0 / (x - self.x2)) * (d1 - self.d2)
        };

        self.d2 = d1;
        self.x2 = self.x1;
        self.x1 = x;
        self.ad2_x1 = ad2_x0;

        y
    }
}
