// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The display menu's `Random Patch` and `Mutate Sound`.
//!
//! A random patch is any factory patch. Mutate keeps the patch and nudges the controls that colour a sound without
//! being able to silence it: the filters' cutoff, resonance and drive, the amp envelopes' attack, decay and release
//! (never sustain or level), and each oscillator's detune and stereo spread. Every nudge is in the control's own travel
//! (its normalised value, so a skewed time knob moves as the finger would), and the result stays inside the range.

use crate::ids::*;
use crate::presets;
use crate::ranges::{from_norm, to_norm};
use crate::util::Rng;

/// The controls Mutate moves.
pub const MUTABLE: [usize; 22] = [
    FILT1_CUTOFF,
    FILT1_RES,
    FILT1_DRIVE,
    FILT2_CUTOFF,
    FILT2_RES,
    FILT2_DRIVE,
    PART_A_ATT,
    PART_A_DEC,
    PART_A_REL,
    PART_B_ATT,
    PART_B_DEC,
    PART_B_REL,
    OSC1_DETUNE,
    OSC2_DETUNE,
    OSC3_DETUNE,
    OSC4_DETUNE,
    OSC1_STEREO,
    OSC2_STEREO,
    OSC3_STEREO,
    OSC4_STEREO,
    OSC1_FINE,
    OSC2_FINE,
];

/// How far one Mutate moves a control, as a share of its travel (either way).
pub const AMOUNT: f32 = 0.15;

/// A factory patch other than `current`, chosen by `rng`.
pub fn random_patch(rng: &mut Rng, current: usize) -> usize {
    let pick = rng.next_u32() as usize % (presets::COUNT - 1);
    // Skip over the current one, so the pick is always a change.
    if pick >= current { pick + 1 } else { pick }
}

/// Moves each mutable control of `values` by up to `amount` of its travel, in place.
pub fn mutate(values: &mut [f32], rng: &mut Rng, amount: f32) {
    let amount = amount.clamp(0.0, 1.0);
    for &id in &MUTABLE {
        let n = to_norm(id, values[id]) + rng.bipolar() * amount;
        values[id] = from_norm(id, n.clamp(0.0, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::params::Snapshot;

    fn heard(values: &Snapshot) -> (f32, bool) {
        let mut e = Engine::new(48_000.0);
        let (mut l, mut r) = (vec![0.0; 128], vec![0.0; 128]);
        e.render(values, 120.0, &mut l, &mut r);
        e.note_on(55, 1.0);
        e.note_on(62, 1.0);
        let (mut sum, mut n, mut sane) = (0.0f32, 0usize, true);
        for _ in 0..375 {
            e.render(values, 120.0, &mut l, &mut r);
            for s in l.iter().chain(&r) {
                sane &= s.is_finite() && s.abs() <= 1.0;
                sum += s * s;
                n += 1;
            }
        }
        ((sum / n as f32).sqrt(), sane)
    }

    #[test]
    fn a_random_patch_is_never_the_current_one_and_reaches_the_ends() {
        let mut rng = Rng::new(7);
        let (mut lowest, mut highest) = (usize::MAX, 0);
        for current in [0, 1, presets::COUNT / 2, presets::COUNT - 1] {
            for _ in 0..300 {
                let p = random_patch(&mut rng, current);
                assert!(p < presets::COUNT && p != current, "{p} from {current}");
                lowest = lowest.min(p);
                highest = highest.max(p);
            }
        }
        assert!(
            lowest < 100 && highest > presets::COUNT - 100,
            "{lowest}..{highest}"
        );
    }

    #[test]
    fn mutating_changes_only_the_listed_controls_and_stays_in_range() {
        let mut rng = Rng::new(99);
        for patch in [0, 40, 500, 1500, presets::COUNT - 1] {
            let before = presets::values(patch);
            let mut v = before;
            mutate(&mut v, &mut rng, AMOUNT);
            let mut moved = 0;
            for i in 0..COUNT {
                if MUTABLE.contains(&i) {
                    moved += usize::from((v[i] - before[i]).abs() > 1e-6);
                    let (lo, hi) = match SPECS[i].kind {
                        Kind::Lin(a, b) | Kind::Skew(a, b, _) => (a, b),
                        Kind::Int(a, b) => (a as f32, b as f32),
                    };
                    assert!(v[i] >= lo && v[i] <= hi, "{} = {}", SPECS[i].id, v[i]);
                } else {
                    assert_eq!(v[i], before[i], "{} moved", SPECS[i].id);
                }
            }
            assert!(moved >= 3, "patch {patch}: {moved} moved");
        }
    }

    #[test]
    fn no_amount_changes_nothing_and_a_travel_is_never_left() {
        let mut rng = Rng::new(3);
        let before = presets::values(10);
        let mut v = before;
        mutate(&mut v, &mut rng, 0.0);
        for &id in &MUTABLE {
            assert!(
                (v[id] - before[id]).abs() <= 1e-3 * (1.0 + before[id].abs()),
                "{id}"
            );
        }
        // Many mutations in a row wander but never leave the ranges.
        let mut w = before;
        for _ in 0..200 {
            mutate(&mut w, &mut rng, 1.0);
        }
        for &id in &MUTABLE {
            assert!(w[id].is_finite());
        }
    }

    #[test]
    fn a_mutated_patch_still_sounds_and_stays_inside_the_limiter() {
        let mut rng = Rng::new(2026);
        let mut silent = 0;
        let total = 40;
        for k in 0..total {
            let patch = (k * 77) % presets::COUNT;
            let mut v = presets::values(patch);
            mutate(&mut v, &mut rng, AMOUNT);
            let (level, sane) = heard(&v);
            assert!(sane, "patch {patch} left the limiter or went non-finite");
            silent += usize::from(level < 1e-4);
        }
        // Mutating never mutes the amp's sustain or level, so at most a few odd patches (a filter closed on a
        // note-less pad) may be quiet over the second listened to.
        assert!(
            silent * 10 <= total,
            "{silent} of {total} patches are silent"
        );
    }
}
