//! Zero-allocation Ableton Live-style `Utility` stereo processor
//! (Phase Invert `L`/`R`, Mid/Side Width `0%..=500%`, Balance/Pan `50L..50R`, Gain `-60..=+60 dB`).

use super::flush_denormal;

/// Normalized `P1` position corresponding to exact `100%` (`1.0x`) stereo width.
pub const UTILITY_WIDTH_DEFAULT_NORM: f32 = 0.40;

const WIDTH_SPLIT_NORM: f32 = 0.68;
// Chosen so `WIDTH_SPLIT_NORM * (0.40 / 0.68)^p = 100%` (`1.0` linear side gain at norm=0.40).
const WIDTH_LOW_EXP: f32 = 1.306_277_9;

/// Maps normalized `P1` (`0.0..=1.0`) to Side width multiplier (`0.0..=5.0`, `0%..=500%`).
///
/// Gives `0%..=200%` the first `68%` of knob travel (`100%` at `0.40`) for fine `1%` adjustments,
/// and ramps smoothly from `200%` to `500%` over the top `32%`.
#[inline]
pub fn utility_width_from_norm(norm: f32) -> f32 {
    let n = norm.clamp(0.0, 1.0);
    if n <= WIDTH_SPLIT_NORM {
        2.0 * (n / WIDTH_SPLIT_NORM).powf(WIDTH_LOW_EXP)
    } else {
        let t = (n - WIDTH_SPLIT_NORM) / (1.0 - WIDTH_SPLIT_NORM);
        2.0 + 3.0 * (0.41 * t + 0.59 * t * t)
    }
}

/// Maps normalized `P3` (`0.0..=1.0`, center `0.5`) to Gain (`-60.0..=+60.0 dB`) via mixed
/// linear + cubic response so `-20..=+20 dB` spans ~65% of physical knob turn.
#[inline]
pub fn utility_gain_db_from_norm(norm: f32) -> f32 {
    let s = (norm.clamp(0.0, 1.0) - 0.5) * 2.0;
    60.0 * (0.22 * s + 0.78 * s * s * s)
}

/// Maps normalized `P2` (`0.0..=1.0`) to bipolar stereo balance (`-1.0 [50L]` .. `+1.0 [50R]`).
#[inline]
pub fn utility_pan_bipolar(norm: f32) -> f32 {
    norm.clamp(0.0, 1.0) * 2.0 - 1.0
}

/// Formats normalized `P2` as Ableton-style balance label (`50L`..`C`..`50R`).
pub fn format_utility_pan(norm: f32) -> String {
    let v = (utility_pan_bipolar(norm) * 50.0).round() as i32;
    if v == 0 {
        "C".to_string()
    } else if v < 0 {
        format!("{}L", -v)
    } else {
        format!("{v}R")
    }
}

/// Zero-allocation per-stage stereo `Utility` processor with `1.5 ms` parameter smoothing.
pub struct UtilityStereo {
    width_sm: f32,
    pan_sm: f32,
    gain_lin_sm: f32,
    smooth_coeff: f32,
    initialized: bool,
}

impl UtilityStereo {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            width_sm: 1.0,
            pan_sm: 0.0,
            gain_lin_sm: 1.0,
            smooth_coeff: 0.02,
            initialized: false,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.smooth_coeff = 1.0 - (-1.0 / (0.0015 * sr)).exp();
    }

    pub fn reset(&mut self) {
        self.width_sm = 1.0;
        self.pan_sm = 0.0;
        self.gain_lin_sm = 1.0;
        self.initialized = false;
    }

    #[inline]
    pub fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        inv_l: bool,
        inv_r: bool,
    ) -> (f32, f32, f32) {
        let target_width = utility_width_from_norm(p[0]);
        let target_pan = utility_pan_bipolar(p[1]);
        let gain_db = utility_gain_db_from_norm(p[2]);
        let target_gain_lin = 10.0_f32.powf(gain_db * 0.05);
        let mix = p[3].clamp(0.0, 1.0);

        if !self.initialized {
            self.width_sm = target_width;
            self.pan_sm = target_pan;
            self.gain_lin_sm = target_gain_lin;
            self.initialized = true;
        } else {
            let c = self.smooth_coeff;
            self.width_sm += c * (target_width - self.width_sm);
            self.pan_sm += c * (target_pan - self.pan_sm);
            self.gain_lin_sm += c * (target_gain_lin - self.gain_lin_sm);
        }

        // 1. Phase Invert (L / R)
        let l_inv = if inv_l { -in_l } else { in_l };
        let r_inv = if inv_r { -in_r } else { in_r };

        // 2. Mid/Side Width (0% Mono .. 100% unity .. 500% super-wide)
        let mid = (l_inv + r_inv) * 0.5;
        let side = (l_inv - r_inv) * 0.5 * self.width_sm;
        let mut l = mid + side;
        let mut r = mid - side;

        // 3. Stereo Balance / Pan (-1.0 = 50L .. +1.0 = 50R)
        let bal_l = (1.0 - self.pan_sm).clamp(0.0, 1.0);
        let bal_r = (1.0 + self.pan_sm).clamp(0.0, 1.0);
        l *= bal_l;
        r *= bal_r;

        // 4. Gain (-60..=+60 dB) & Wet/Dry Mix
        l *= self.gain_lin_sm;
        r *= self.gain_lin_sm;

        let out_l = flush_denormal(in_l + mix * (l - in_l));
        let out_r = flush_denormal(in_r + mix * (r - in_r));
        let act = if out_l.abs().max(out_r.abs()) > 1e-5 {
            ((gain_db.abs() / 24.0) + (self.width_sm - 1.0).abs() * 0.4 + self.pan_sm.abs() * 0.5)
                .clamp(0.15, 1.0)
                * mix
        } else {
            0.0
        };
        (out_l, out_r, act)
    }
}
