//! The peak hold and the clip lamp of a level meter, as a mixing console has them: the highest peak of the last moments stays
//! visible as a line over the meter, then falls; a peak at full scale lights a lamp that stays lit until it is cleared.

/// How long a peak stays where it was, in seconds.
pub const HOLD_SECONDS: f32 = 1.5;
/// How fast it falls afterwards, in meter-law units per second (the meter's full height is 1).
pub const FALL_PER_SECOND: f32 = 0.6;
/// A peak at or above this level (digital full scale) is a clip.
pub const CLIP_LEVEL: f32 = 1.0;

/// One strip's two meters (left, right).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PeakHold {
    held: [f32; 2],
    age: [f32; 2],
    clipped: bool,
}

impl PeakHold {
    /// Takes this frame's `peaks` (linear, as the meters give them) after `dt` seconds.
    pub fn feed(&mut self, peaks: [f32; 2], dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.5)
        } else {
            0.0
        };
        for (k, &raw) in peaks.iter().enumerate() {
            let p = if raw.is_finite() { raw.max(0.0) } else { 0.0 };
            if p >= self.held[k] {
                self.held[k] = p;
                self.age[k] = 0.0;
            } else {
                self.age[k] += dt;
                if self.age[k] > HOLD_SECONDS {
                    // The fall is in the meter's own law, so it looks even whatever the level.
                    let m = crate::ui::mini_mixer::meter(self.held[k]) - FALL_PER_SECOND * dt;
                    self.held[k] = if m <= 0.0 { 0.0 } else { from_meter(m) };
                    if self.held[k] < p {
                        self.held[k] = p;
                        self.age[k] = 0.0;
                    }
                }
            }
            if p >= CLIP_LEVEL {
                self.clipped = true;
            }
        }
    }

    /// The held peaks (linear).
    pub fn held(&self) -> [f32; 2] {
        self.held
    }

    /// A peak reached full scale since the lamp was cleared.
    pub fn clipped(&self) -> bool {
        self.clipped
    }

    /// Puts the lamp out.
    pub fn clear_clip(&mut self) {
        self.clipped = false;
    }
}

/// The linear peak a meter-law value `m` stands for: the inverse of [`crate::ui::mini_mixer::meter`].
pub fn from_meter(m: f32) -> f32 {
    if m <= 0.0 {
        return 0.0;
    }
    let db = m.min(1.0).powf(1.0 / 1.6667) * 66.0 - 60.0;
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::mini_mixer::meter;

    #[test]
    fn the_inverse_of_the_meter_law_round_trips() {
        for p in [0.002f32, 0.01, 0.1, 0.5, 0.9, 1.0] {
            let back = from_meter(meter(p));
            assert!((back / p - 1.0).abs() < 0.01, "{p} -> {back}");
        }
        assert_eq!(from_meter(0.0), 0.0);
    }

    #[test]
    fn a_peak_is_held_then_falls_and_a_louder_one_takes_over() {
        let mut h = PeakHold::default();
        h.feed([0.8, 0.2], 0.016);
        assert_eq!(h.held(), [0.8, 0.2]);
        // A quiet signal under the hold time changes nothing.
        for _ in 0..50 {
            h.feed([0.1, 0.1], 0.016);
        }
        assert_eq!(h.held(), [0.8, 0.2]);
        // After the hold time it falls, monotonically, to the signal.
        let mut prev = 0.8;
        for _ in 0..600 {
            h.feed([0.1, 0.1], 0.016);
            assert!(h.held()[0] <= prev + 1e-6);
            prev = h.held()[0];
        }
        assert!((h.held()[0] - 0.1).abs() < 1e-6 && (h.held()[1] - 0.1).abs() < 1e-6);
        h.feed([0.5, 0.0], 0.016);
        assert_eq!(h.held()[0], 0.5);
    }

    #[test]
    fn full_scale_lights_the_clip_lamp_until_it_is_cleared() {
        let mut h = PeakHold::default();
        h.feed([0.99, 0.5], 0.016);
        assert!(!h.clipped());
        h.feed([0.2, 1.0], 0.016);
        assert!(h.clipped());
        for _ in 0..1000 {
            h.feed([0.0, 0.0], 0.016);
        }
        assert!(h.clipped(), "the lamp stays lit");
        h.clear_clip();
        assert!(!h.clipped());
    }

    #[test]
    fn bad_input_never_breaks_the_hold() {
        let mut h = PeakHold::default();
        h.feed([f32::NAN, f32::INFINITY], 0.016);
        h.feed([-1.0, 0.3], f32::NAN);
        assert!(h.held().iter().all(|v| v.is_finite() && *v >= 0.0));
        h.feed([0.0, 0.0], 100.0);
        assert!(h.held().iter().all(|v| v.is_finite()));
    }
}
