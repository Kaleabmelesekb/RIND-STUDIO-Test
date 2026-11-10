// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rind's factory patches: the patch bar's names and the parameter values each one sets (on top of the defaults).

use crate::ids::*;
use crate::params::Snapshot;

pub const COUNT: usize = 8;

const NAMES: [&str; COUNT] = [
    "ARP 303 Saw",
    "Init Saw",
    "Warm Pad",
    "Sub Bass",
    "Pluck Lead",
    "Glass Keys",
    "Wobble Mono",
    "Soft Strings",
];

/// The patch bar's text for patch `i`.
pub fn name(i: usize) -> String {
    let i = i.min(COUNT - 1);
    format!("{:03}: {}", i + 1, NAMES[i])
}

fn osc_at(n: usize, f: usize) -> usize {
    OSC_BASE + n * OSC_STRIDE + f
}
fn part_at(n: usize, f: usize) -> usize {
    PART_BASE + n * PART_STRIDE + f
}
fn filt_at(n: usize, f: usize) -> usize {
    FILT_BASE + n * FILT_STRIDE + f
}

/// The parameters patch `i` sets; the rest take their defaults.
pub fn overrides(i: usize) -> Vec<(usize, f32)> {
    let mut v: Vec<(usize, f32)> = Vec::new();
    let amp = |v: &mut Vec<(usize, f32)>, a: f32, d: f32, s: f32, r: f32| {
        for (f, x) in [
            (part::ATT, a),
            (part::DEC, d),
            (part::SUS, s),
            (part::REL, r),
        ] {
            v.push((part_at(0, f), x));
        }
    };
    match i.min(COUNT - 1) {
        0 => {
            v.push((ARP_ON, 1.0));
            v.push((EQ_ON, 1.0));
            v.push((DLY_ON, 1.0));
            v.push((osc_at(0, osc::WAVE), 1.0));
            v.push((osc_at(0, osc::OCT), -1.0));
            v.push((osc_at(0, osc::VOICES), 2.0));
            v.push((osc_at(1, osc::VOICES), 0.0));
            v.push((ARP_OCT, 1.0));
            v.push((ARP_GATE, 0.6));
            v.push((filt_at(0, filt::CUTOFF), 4000.0));
            v.push((filt_at(0, filt::RES), 0.3));
            amp(&mut v, 0.003, 0.3, 0.6, 0.2);
            let tr = [12.0, 0.0, 12.0, 12.0, 0.0, 12.0, 0.0, 17.0];
            let vel = [58.0, 81.0, 49.0, 100.0, 118.0, 78.0, 110.0, 114.0];
            for k in 0..8 {
                v.push((ARP_TR1 + k, tr[k]));
                v.push((ARP_STEP1 + k, vel[k] / 127.0));
            }
            for k in [1, 4, 7] {
                v.push((ARP_HOLD1 + k, 1.0));
            }
        }
        1 => {}
        2 => {
            for n in 0..2 {
                v.push((osc_at(n, osc::WAVE), 1.0));
                v.push((osc_at(n, osc::VOICES), 4.0));
                v.push((osc_at(n, osc::DETUNE), 0.35 + 0.05 * n as f32));
            }
            amp(&mut v, 0.8, 0.5, 0.8, 1.2);
            v.push((filt_at(0, filt::CUTOFF), 2500.0));
            v.push((REV_ON, 1.0));
            v.push((CHO_ON, 1.0));
        }
        3 => {
            v.push((MONO, 1.0));
            v.push((WARM, 1.0));
            v.push((PORTA, 0.05));
            v.push((osc_at(0, osc::WAVE), 1.0));
            v.push((osc_at(0, osc::OCT), -1.0));
            v.push((osc_at(0, osc::VOICES), 1.0));
            v.push((osc_at(1, osc::WAVE), 0.0));
            v.push((osc_at(1, osc::OCT), -2.0));
            v.push((osc_at(1, osc::VOICES), 1.0));
            v.push((filt_at(0, filt::CUTOFF), 900.0));
            v.push((filt_at(0, filt::RES), 0.25));
            amp(&mut v, 0.003, 0.4, 0.8, 0.15);
        }
        4 => {
            v.push((osc_at(0, osc::WAVE), 1.0));
            v.push((osc_at(0, osc::VOICES), 3.0));
            v.push((osc_at(0, osc::DETUNE), 0.15));
            amp(&mut v, 0.003, 0.25, 0.0, 0.2);
            v.push((filt_at(0, filt::CUTOFF), 3000.0));
            v.push((filt_at(0, filt::RES), 0.4));
            v.push((DLY_ON, 1.0));
        }
        5 => {
            v.push((osc_at(0, osc::WAVE), 2.0));
            v.push((osc_at(0, osc::VOICES), 2.0));
            v.push((osc_at(1, osc::WAVE), 2.0));
            v.push((osc_at(1, osc::OCT), 1.0));
            amp(&mut v, 0.003, 0.6, 0.3, 0.5);
            v.push((REV_ON, 1.0));
        }
        6 => {
            v.push((MONO, 1.0));
            v.push((osc_at(0, osc::WAVE), 1.0));
            v.push((osc_at(0, osc::VOICES), 2.0));
            v.push((filt_at(0, filt::RES), 0.5));
            v.push((filt_at(0, filt::CUTOFF), 1200.0));
            v.push((LFO_BASE + lfo::RATE, 2.5));
            let slot = MAT_BASE + 4 * MAT_STRIDE;
            v.push((slot + mat::SRC, msrc::LFO_1 as f32));
            v.push((slot + mat::DST, mtgt::CUTOFF_1 as f32));
            v.push((slot + mat::AMT, 0.6));
        }
        _ => {
            for n in 0..2 {
                v.push((osc_at(n, osc::WAVE), 1.0));
                v.push((osc_at(n, osc::VOICES), 6.0));
                v.push((osc_at(n, osc::DETUNE), 0.3 - 0.05 * n as f32));
            }
            amp(&mut v, 0.5, 0.5, 0.9, 0.8);
            v.push((filt_at(0, filt::CUTOFF), 3500.0));
            v.push((CHO_ON, 1.0));
        }
    }
    v
}

/// Every parameter's plain value for patch `i`: the defaults with its overrides and the patch number.
pub fn values(i: usize) -> Snapshot {
    let mut s = crate::params::RindParams::defaults();
    for (p, x) in overrides(i) {
        s[p] = x;
    }
    s[PATCH] = i.min(COUNT - 1) as f32;
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_patch_sets_only_values_inside_their_ranges() {
        for i in 0..COUNT {
            for (p, x) in overrides(i) {
                let ok = match SPECS[p].kind {
                    Kind::Lin(lo, hi) | Kind::Skew(lo, hi, _) => x >= lo && x <= hi,
                    Kind::Int(lo, hi) => x.fract() == 0.0 && x >= lo as f32 && x <= hi as f32,
                };
                assert!(ok, "patch {i}: {} = {x}", SPECS[p].id);
            }
        }
    }

    #[test]
    fn the_bar_reads_like_the_photo() {
        assert_eq!(name(0), "001: ARP 303 Saw");
        assert_eq!(PATCH, SPECS.iter().position(|s| s.id == "patch").unwrap());
        assert_eq!(
            COUNT as i32 - 1,
            match SPECS[PATCH].kind {
                Kind::Int(_, hi) => hi,
                _ => -1,
            }
        );
    }
}
