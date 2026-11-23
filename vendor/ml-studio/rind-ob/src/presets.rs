// SPDX-License-Identifier: GPL-3.0-or-later
//! The patches OB-Xf ships (493, all CC0), as `gen.py` made them from its `.fxp` files: every setting of a patch as
//! the value Rind OB stores, so a patch loads as it is.
use std::sync::OnceLock;

static BIN: &[u8] = include_bytes!("../banks/presets.bin");

pub struct Preset {
    pub category: usize,
    pub author: usize,
    pub name: String,
    /// One stored value (0..=1000) for each of the settings.
    pub values: Vec<u16>,
}

pub struct Bank {
    pub categories: Vec<String>,
    pub authors: Vec<String>,
    pub presets: Vec<Preset>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn string(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        self.take(n).map(|s| String::from_utf8_lossy(s).into_owned())
    }
}

fn parse(b: &[u8]) -> Option<Bank> {
    let mut r = Reader { b, at: 0 };
    let (params, cats, authors, count) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
    let categories = (0..cats).map(|_| r.string()).collect::<Option<Vec<_>>>()?;
    let authors = (0..authors).map(|_| r.string()).collect::<Option<Vec<_>>>()?;
    let mut presets = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let category = usize::from(r.u8()?);
        let author = usize::from(r.u8()?);
        let name = r.string()?;
        let values = (0..params).map(|_| r.u16()).collect::<Option<Vec<_>>>()?;
        presets.push(Preset { category, author, name, values });
    }
    Some(Bank { categories, authors, presets })
}

pub fn bank() -> &'static Bank {
    static BANK: OnceLock<Bank> = OnceLock::new();
    BANK.get_or_init(|| parse(BIN).unwrap_or(Bank { categories: Vec::new(), authors: Vec::new(), presets: Vec::new() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_is_whole() {
        let b = bank();
        assert_eq!(b.presets.len(), 493);
        assert_eq!(b.categories.len(), 18);
        for p in &b.presets {
            assert_eq!(p.values.len(), crate::settings::COUNT);
            assert!(p.values.iter().all(|&v| v <= 1000), "{}", p.name);
            assert!(p.category < b.categories.len() && p.author < b.authors.len());
        }
    }
}
