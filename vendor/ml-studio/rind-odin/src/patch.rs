// SPDX-License-Identifier: GPL-3.0-or-later
//! Odin 2's patch files: a JUCE `ValueTree` in binary (what `writeToStream` makes). The root is "Odin"; its children
//! `PARAM` hold a parameter each (`id`, `value`), and `osc`, `misc`, `mod`, `lfo`, `fx`, `draw` hold the properties that
//! are not parameters. The engine loads a patch itself; this reads the settings out of one for the editor.
use crate::settings::{COUNT, INFO, Kind, PICK, SETTINGS};
use std::collections::HashMap;

/// The newest `patch_migration_version` the engine reads (`ODIN_PATCH_MIGRATION_VERSION`).
const MIGRATION: i64 = 5;

#[derive(Clone, Debug)]
pub struct Patch {
    pub name: String,
    /// The setting values the patch means.
    pub values: Vec<i32>,
}

enum Var {
    Num(f64),
    Text(String),
    None,
}
impl Var {
    fn num(&self) -> Option<f64> {
        match self {
            Var::Num(n) => Some(*n),
            _ => None,
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}
impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let v = *self.b.get(self.i)?;
        self.i += 1;
        Some(v)
    }
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.i..self.i.checked_add(n)?)?;
        self.i += n;
        Some(s)
    }
    /// JUCE's compressed int: the low seven bits of the first byte count the bytes after it, its top bit is the sign.
    fn int(&mut self) -> Option<i64> {
        let h = self.byte()?;
        let n = usize::from(h & 0x7f);
        if n > 8 {
            return None;
        }
        let mut v = 0_i64;
        for k in 0..n {
            v |= i64::from(self.byte()?) << (8 * k);
        }
        Some(if h & 0x80 != 0 { -v } else { v })
    }
    fn string(&mut self) -> Option<String> {
        let rest = self.b.get(self.i..)?;
        let n = rest.iter().position(|&c| c == 0)?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.i += n + 1;
        Some(s)
    }
    fn var(&mut self) -> Option<Var> {
        let n = usize::try_from(self.int()?).ok()?;
        if n == 0 {
            return Some(Var::None);
        }
        let kind = self.byte()?;
        let data = self.take(n - 1)?;
        Some(match kind {
            1 if data.len() == 4 => Var::Num(f64::from(i32::from_le_bytes(data.try_into().ok()?))),
            2 => Var::Num(1.0),
            3 => Var::Num(0.0),
            4 if data.len() == 8 => Var::Num(f64::from_le_bytes(data.try_into().ok()?)),
            5 => Var::Text(String::from_utf8_lossy(data.strip_suffix(&[0]).unwrap_or(data)).into_owned()),
            6 if data.len() == 8 => Var::Num(i64::from_le_bytes(data.try_into().ok()?) as f64),
            _ => Var::None,
        })
    }
}

struct Tree {
    name: String,
    props: Vec<(String, Var)>,
    children: Vec<Tree>,
}

fn tree(r: &mut Reader, depth: u32) -> Option<Tree> {
    if depth > 6 {
        return None;
    }
    let name = r.string()?;
    let n = usize::try_from(r.int()?).ok()?;
    // A property takes at least two bytes: a count over what is left is not a patch.
    if n > r.b.len() {
        return None;
    }
    let mut props = Vec::with_capacity(n);
    for _ in 0..n {
        let k = r.string()?;
        props.push((k, r.var()?));
    }
    let n = usize::try_from(r.int()?).ok()?;
    if n > r.b.len() {
        return None;
    }
    let mut children = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        children.push(tree(r, depth + 1)?);
    }
    Some(Tree { name, props, children })
}

/// The engine's value of setting `k` for the setting's own value `v`.
pub fn real(k: usize, v: i32) -> f64 {
    let i = &INFO[k];
    match i.kind {
        Kind::Float => {
            let n = (f64::from(v) / 1000.).clamp(0., 1.);
            let n = if i.skew != 1. && n > 0. { n.powf(1. / f64::from(i.skew)) } else { n };
            f64::from(i.lo) + (f64::from(i.hi) - f64::from(i.lo)) * n
        }
        Kind::Int | Kind::Prop => f64::from(v),
        Kind::Pick => PICK[usize::from(i.table)].get(v.max(0) as usize).map_or(0., |t| f64::from(t.0)),
        Kind::Amount => f64::from(v) / 1000.,
    }
}

/// The setting value for what the engine holds as `x`.
pub fn setting_of(k: usize, x: f64) -> i32 {
    let i = &INFO[k];
    let s = &SETTINGS[k];
    let v = match i.kind {
        Kind::Float => {
            let n = ((x - f64::from(i.lo)) / (f64::from(i.hi) - f64::from(i.lo))).clamp(0., 1.);
            let n = if i.skew != 1. && n > 0. { n.powf(f64::from(i.skew)) } else { n };
            (n * 1000.).round() as i32
        }
        Kind::Int | Kind::Prop => x.round() as i32,
        Kind::Amount => (x * 1000.).round() as i32,
        Kind::Pick => {
            let table = PICK[usize::from(i.table)];
            let id = x.round() as i32;
            match table.iter().position(|t| t.0 == id) {
                Some(p) => p as i32,
                // The matrix's "Off" is 1 in Odin's editor and 0 in a new patch; a wavetable is its number from 1.
                None if matches!(i.table, 2 | 3) => 0,
                None => s.start,
            }
        }
    };
    v.clamp(s.range.0, s.range.1)
}

/// Reads an Odin 2 patch.
pub fn parse(data: &[u8]) -> Result<Patch, &'static str> {
    let mut r = Reader { b: data, i: 0 };
    let root = tree(&mut r, 0).ok_or("NOT AN ODIN 2 PATCH")?;
    if root.name != "Odin" {
        return Err("NOT AN ODIN 2 PATCH");
    }
    let mut params: HashMap<&str, f64> = HashMap::new();
    let mut props: HashMap<(&str, &str), f64> = HashMap::new();
    let mut name = String::new();
    for c in &root.children {
        if c.name == "PARAM" {
            let (mut id, mut value) = (None, None);
            for (k, v) in &c.props {
                match (k.as_str(), v) {
                    ("id", Var::Text(s)) => id = Some(s.as_str()),
                    ("value", v) => value = v.num(),
                    _ => {}
                }
            }
            if let (Some(id), Some(value)) = (id, value) {
                params.insert(id, value);
            }
        } else {
            for (k, v) in &c.props {
                if let Some(n) = v.num() {
                    props.insert((c.name.as_str(), k.as_str()), n);
                }
                if let (Var::Text(s), "misc", "patch_name") = (v, c.name.as_str(), k.as_str()) {
                    name.clone_from(s);
                }
            }
        }
    }
    if let Some(&v) = props.get(&("misc", "patch_migration_version")) {
        if v as i64 > MIGRATION {
            return Err("MADE BY A NEWER ODIN 2");
        }
    }
    if params.is_empty() {
        return Err("NOT AN ODIN 2 PATCH");
    }
    let values = (0..COUNT)
        .map(|k| {
            let i = &INFO[k];
            let x = if i.tree.is_empty() { params.get(i.key).copied() } else { props.get(&(i.tree, i.key)).copied() };
            x.map_or(SETTINGS[k].start, |x| setting_of(k, x))
        })
        .collect();
    Ok(Patch { name, values })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_factory_patch_reads() {
        let bank = crate::presets::bank();
        assert_eq!(bank.patches.len(), 260);
        for p in &bank.patches {
            let patch = parse(p.data).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            assert_eq!(patch.values.len(), COUNT);
            for (k, &v) in patch.values.iter().enumerate() {
                let (lo, hi) = SETTINGS[k].range;
                assert!((lo..=hi).contains(&v), "{} setting {}", p.name, SETTINGS[k].id);
            }
        }
    }

    #[test]
    fn real_and_setting_agree() {
        for k in 0..COUNT {
            let s = &SETTINGS[k];
            for v in [s.range.0, s.start, s.range.1, (s.range.0 + s.range.1) / 2] {
                assert_eq!(setting_of(k, real(k, v)), v, "{}", s.id);
            }
        }
    }

    #[test]
    fn junk_is_refused() {
        assert!(parse(&[]).is_err());
        assert!(parse(b"hello, this is not a patch").is_err());
        assert!(parse(&[0xff; 64]).is_err());
    }
}
