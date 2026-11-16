// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rind's factory patches: the patch bar's names, the categories the preset browser lists them in, and the parameter
//! values each one sets (on top of the defaults).
//!
//! A patch is a [`Recipe`]: a short list of the settings that make a sound (the oscillators of part A, its envelope, the
//! filter and its envelope, one LFO, the effects, the arpeggiator), which [`overrides`] turns into parameter values.
//! The table (`preset_table.rs`) is written by `tools/gen_rind_presets.py`.

use std::ops::Range;

use crate::ids::*;
use crate::params::Snapshot;
use crate::preset_table::TABLE;

pub use crate::preset_table::CATEGORIES;

/// How many patches there are; the patch parameter counts them.
pub const COUNT: usize = TABLE.len();

/// One factory patch: its category (an index into [`CATEGORIES`]), its name and what it sets.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub cat: u8,
    pub name: &'static str,
    pub recipe: Recipe,
}

/// The settings of a patch. Oscillator 1 and 2 are part A's; `l2` is oscillator 2's volume. `lt` is the modulation
/// target the LFO drives ([`mtgt`]) by `la`; `fe` is how far the filter envelope (envelope 3) opens the filter (±1 is
/// five octaves); `vib` is a vibrato of the second LFO. `arp` is 0 for off, else the arpeggiator mode + 1; `pat` picks
/// one of the arpeggiator's step patterns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub w1: u8,
    pub v1: u8,
    pub d1: f32,
    pub o1: i8,
    pub w2: u8,
    pub v2: u8,
    pub d2: f32,
    pub o2: i8,
    pub s2: i8,
    pub l2: f32,
    pub st: f32,
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
    pub ft: u8,
    pub fs: u8,
    pub cut: f32,
    pub res: f32,
    pub drv: f32,
    pub key: f32,
    pub fe: f32,
    pub ea: f32,
    pub ed: f32,
    pub es: f32,
    pub er: f32,
    pub lw: u8,
    pub lr: f32,
    pub lt: u8,
    pub la: f32,
    pub vib: f32,
    pub mono: u8,
    pub porta: f32,
    pub dty: u8,
    pub dist: f32,
    pub cho: f32,
    pub phs: f32,
    pub dly: f32,
    pub dfb: f32,
    pub rev: f32,
    pub rsz: f32,
    pub bass: f32,
    pub treb: f32,
    pub comp: u8,
    pub arp: u8,
    pub div: u8,
    pub aoct: u8,
    pub gate: f32,
    pub pat: u8,
}

/// The plain patch: what a field left out of a table row means.
pub const BASE: Recipe = Recipe {
    w1: 1,
    v1: 1,
    d1: 0.2,
    o1: 0,
    w2: 1,
    v2: 0,
    d2: 0.2,
    o2: 0,
    s2: 0,
    l2: 0.75,
    st: 0.5,
    a: 0.003,
    d: 0.6,
    s: 1.0,
    r: 0.25,
    ft: 3,
    fs: 1,
    cut: 12000.0,
    res: 0.1,
    drv: 0.0,
    key: 0.0,
    fe: 0.0,
    ea: 0.003,
    ed: 0.4,
    es: 0.5,
    er: 0.3,
    lw: 0,
    lr: 2.0,
    lt: 0,
    la: 0.0,
    vib: 0.0,
    mono: 0,
    porta: 0.0,
    dty: 0,
    dist: 0.0,
    cho: 0.0,
    phs: 0.0,
    dly: 0.0,
    dfb: 0.35,
    rev: 0.0,
    rsz: 0.6,
    bass: 0.0,
    treb: 0.0,
    comp: 0,
    arp: 0,
    div: 11,
    aoct: 1,
    gate: 0.5,
    pat: 0,
};

/// The arpeggiator's step patterns: transpose, velocity (of 127) and tie of each of the 16 steps. Pattern 0 is the
/// plain table, which a recipe leaves alone.
const PATTERNS: [([i8; 16], [u8; 16], u16); 6] = [
    ([0; 16], [102; 16], 0),
    (
        [12, 0, 12, 12, 0, 12, 0, 17, 12, 0, 12, 12, 0, 12, 0, 17],
        [
            58, 81, 49, 100, 118, 78, 110, 114, 58, 81, 49, 100, 118, 78, 110, 114,
        ],
        0b0100_1001_0100_1010,
    ),
    (
        [0, 12, 0, 12, 0, 12, 0, 12, 0, 12, 0, 12, 0, 12, 0, 12],
        [
            110, 80, 110, 80, 110, 80, 110, 80, 110, 80, 110, 80, 110, 80, 110, 80,
        ],
        0,
    ),
    (
        [0; 16],
        [
            127, 40, 90, 40, 127, 40, 90, 40, 127, 40, 90, 40, 127, 60, 90, 60,
        ],
        0,
    ),
    (
        [0, 0, 3, 3, 7, 7, 12, 12, 0, 0, 3, 3, 7, 7, 12, 12],
        [
            110, 70, 100, 70, 110, 70, 100, 70, 110, 70, 100, 70, 110, 70, 100, 70,
        ],
        0,
    ),
    (
        [0, 0, 12, 0, 0, 12, 0, 0, 12, 0, 0, 7, 0, 12, 0, 0],
        [
            120, 60, 100, 60, 120, 60, 100, 60, 120, 60, 100, 60, 120, 60, 100, 60,
        ],
        0b0000_1001_0010_0100,
    ),
];

/// The patch bar's text for patch `i`.
pub fn name(i: usize) -> String {
    format!("{:03}: {}", i.min(COUNT - 1) + 1, title(i))
}

/// Patch `i`'s own name.
pub fn title(i: usize) -> &'static str {
    TABLE[i.min(COUNT - 1)].name
}

/// The category (an index into [`CATEGORIES`]) of patch `i`.
pub fn category(i: usize) -> usize {
    usize::from(TABLE[i.min(COUNT - 1)].cat)
}

/// The patches of category `c`: they are consecutive.
pub fn in_category(c: usize) -> Range<usize> {
    let first = TABLE.iter().position(|p| usize::from(p.cat) == c);
    let n = TABLE.iter().filter(|p| usize::from(p.cat) == c).count();
    first.map_or(0..0, |f| f..f + n)
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
fn lfo_at(n: usize, f: usize) -> usize {
    LFO_BASE + n * LFO_STRIDE + f
}
fn mat_at(slot: usize, f: usize) -> usize {
    MAT_BASE + slot * MAT_STRIDE + f
}

/// The parameters `r` sets; the rest take their defaults.
fn recipe_values(r: &Recipe) -> Vec<(usize, f32)> {
    let mut v: Vec<(usize, f32)> = Vec::with_capacity(80);
    for (n, wave, voices, detune, oct) in [(0, r.w1, r.v1, r.d1, r.o1), (1, r.w2, r.v2, r.d2, r.o2)]
    {
        v.push((osc_at(n, osc::WAVE), f32::from(wave)));
        v.push((osc_at(n, osc::VOICES), f32::from(voices)));
        v.push((osc_at(n, osc::DETUNE), detune));
        v.push((osc_at(n, osc::OCT), f32::from(oct)));
        v.push((osc_at(n, osc::STEREO), r.st));
    }
    v.push((osc_at(1, osc::SEMI), f32::from(r.s2)));
    v.push((osc_at(1, osc::VOL), r.l2));
    for (field, x) in [
        (part::ATT, r.a),
        (part::DEC, r.d),
        (part::SUS, r.s),
        (part::REL, r.r),
    ] {
        v.push((part_at(0, field), x));
    }
    for (field, x) in [
        (filt::TYPE, f32::from(r.ft)),
        (filt::SLOPE, f32::from(r.fs)),
        (filt::CUTOFF, r.cut),
        (filt::RES, r.res),
        (filt::DRIVE, r.drv),
        (filt::KEY, r.key),
    ] {
        v.push((filt_at(0, field), x));
    }
    // The filter envelope: envelope 3 opens (or closes) filter 1.
    if r.fe != 0.0 {
        for (field, x) in [
            (env::ATT, r.ea),
            (env::DEC, r.ed),
            (env::SUS, r.es),
            (env::REL, r.er),
        ] {
            v.push((ENV_BASE + field, x));
        }
        v.push((mat_at(0, mat::SRC), msrc::ENV_3 as f32));
        v.push((mat_at(0, mat::DST), mtgt::CUTOFF_1 as f32));
        v.push((mat_at(0, mat::AMT), r.fe));
    }
    // The LFO's own row of the matrix.
    if r.lt != 0 && r.la != 0.0 {
        v.push((lfo_at(0, lfo::WAVE), f32::from(r.lw)));
        v.push((lfo_at(0, lfo::RATE), r.lr));
        v.push((mat_at(4, mat::SRC), msrc::LFO_1 as f32));
        v.push((mat_at(4, mat::DST), f32::from(r.lt)));
        v.push((mat_at(4, mat::AMT), r.la));
    }
    // A vibrato: the second LFO on the pitch.
    if r.vib > 0.0 {
        v.push((lfo_at(1, lfo::WAVE), 0.0));
        v.push((lfo_at(1, lfo::RATE), 5.5));
        v.push((mat_at(6, mat::SRC), msrc::LFO_2 as f32));
        v.push((mat_at(6, mat::DST), mtgt::PITCH_ALL as f32));
        v.push((mat_at(6, mat::AMT), r.vib));
    }
    if r.mono != 0 {
        v.push((MONO, 1.0));
    }
    if r.porta > 0.0 {
        v.push((PORTA, r.porta));
    }
    if r.dist > 0.0 {
        v.push((DIST_ON, 1.0));
        v.push((DIST_TYPE, f32::from(r.dty)));
        v.push((DIST_DRIVE, r.dist));
    }
    if r.phs > 0.0 {
        v.push((PHS_ON, 1.0));
        v.push((PHS_MIX, r.phs));
    }
    if r.cho > 0.0 {
        v.push((CHO_ON, 1.0));
        v.push((CHO_MIX, r.cho));
    }
    if r.bass != 0.0 || r.treb != 0.0 {
        v.push((EQ_ON, 1.0));
        v.push((EQ_BASS_G, r.bass));
        v.push((EQ_TREB_G, r.treb));
    }
    if r.dly > 0.0 {
        v.push((DLY_ON, 1.0));
        v.push((DLY_MIX, r.dly));
        v.push((DLY_FB, r.dfb));
    }
    if r.rev > 0.0 {
        v.push((REV_ON, 1.0));
        v.push((REV_MIX, r.rev));
        v.push((REV_SIZE, r.rsz));
    }
    if r.comp != 0 {
        v.push((CMP_ON, 1.0));
    }
    if r.arp > 0 {
        v.push((ARP_ON, 1.0));
        v.push((ARP_MODE, f32::from(r.arp - 1)));
        v.push((ARP_DIV, f32::from(r.div)));
        v.push((ARP_OCT, f32::from(r.aoct)));
        v.push((ARP_GATE, r.gate));
        if r.pat > 0 {
            let (tr, vel, tie) = PATTERNS[usize::from(r.pat).min(PATTERNS.len() - 1)];
            for (k, (&t, &vel)) in tr.iter().zip(vel.iter()).enumerate() {
                v.push((ARP_TR1 + k, f32::from(t)));
                v.push((ARP_STEP1 + k, f32::from(vel) / 127.0));
                v.push((ARP_HOLD1 + k, f32::from((tie >> k) & 1)));
            }
        }
    }
    v
}

/// The parameters patch `i` sets; the rest take their defaults.
pub fn overrides(i: usize) -> Vec<(usize, f32)> {
    recipe_values(&TABLE[i.min(COUNT - 1)].recipe)
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
    fn there_are_hundreds_of_patches_in_a_dozen_categories() {
        assert!(std::hint::black_box(COUNT) >= 300, "{COUNT}");
        assert!(std::hint::black_box(CATEGORIES.len()) >= 12);
        for c in 0..CATEGORIES.len() {
            assert!(!in_category(c).is_empty(), "{} is empty", CATEGORIES[c]);
        }
    }

    #[test]
    fn the_categories_cover_the_table_in_order() {
        let mut next = 0;
        for c in 0..CATEGORIES.len() {
            let r = in_category(c);
            assert_eq!(r.start, next, "{}", CATEGORIES[c]);
            assert!(r.clone().all(|i| category(i) == c));
            next = r.end;
        }
        assert_eq!(next, COUNT);
    }

    #[test]
    fn every_patch_has_its_own_name_and_sound() {
        let mut names = std::collections::HashSet::new();
        let mut sounds = std::collections::HashSet::new();
        for i in 0..COUNT {
            assert!(names.insert(title(i)), "{} is twice in the table", title(i));
            let bits: Vec<u32> = overrides(i)
                .iter()
                .map(|&(p, x)| p as u32 ^ x.to_bits())
                .collect();
            assert!(
                sounds.insert(bits),
                "{} sounds like another patch",
                title(i)
            );
        }
    }

    #[test]
    fn every_patch_sets_only_values_inside_their_ranges() {
        for i in 0..COUNT {
            for (p, x) in overrides(i) {
                let ok = match SPECS[p].kind {
                    Kind::Lin(lo, hi) | Kind::Skew(lo, hi, _) => x >= lo && x <= hi,
                    Kind::Int(lo, hi) => x.fract() == 0.0 && x >= lo as f32 && x <= hi as f32,
                };
                assert!(ok, "patch {i} {}: {} = {x}", title(i), SPECS[p].id);
            }
        }
    }

    #[test]
    fn a_patch_that_sets_a_modulation_row_names_the_source_its_panel_stands_for() {
        // Rows 0 and 1 are envelope 3's, 4 and 5 the first LFO's, 6 and 7 the second LFO's.
        for i in 0..COUNT {
            let o = overrides(i);
            let get = |p: usize| o.iter().find(|e| e.0 == p).map(|e| e.1);
            for (slot, src) in [(0, msrc::ENV_3), (4, msrc::LFO_1), (6, msrc::LFO_2)] {
                if get(mat_at(slot, mat::DST)).is_some() {
                    assert_eq!(
                        get(mat_at(slot, mat::SRC)),
                        Some(src as f32),
                        "{}",
                        title(i)
                    );
                }
            }
        }
    }

    #[test]
    fn the_first_patch_is_the_plain_one_and_the_bar_reads_like_the_photo() {
        assert_eq!(name(0), "001: Init Saw");
        assert!(overrides(0).iter().all(|&(p, x)| {
            // The plain recipe sets only what already is the default.
            (SPECS[p].default - x).abs() < 1e-5
        }));
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
