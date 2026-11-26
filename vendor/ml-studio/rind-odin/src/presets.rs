// SPDX-License-Identifier: GPL-3.0-or-later
//! The 260 factory patches of Odin 2, as Odin saves them (`banks/patches.bin`, written by `gen.py`).
use std::sync::OnceLock;

static BANK: &[u8] = include_bytes!("../banks/patches.bin");

pub struct Patch {
    pub category: usize,
    pub name: &'static str,
    /// The patch file as Odin 2 wrote it.
    pub data: &'static [u8],
}

pub struct Bank {
    pub categories: Vec<&'static str>,
    pub patches: Vec<Patch>,
}

fn read() -> Option<Bank> {
    let mut at = 0_usize;
    let mut take = |n: usize| -> Option<&'static [u8]> {
        let s = BANK.get(at..at.checked_add(n)?)?;
        at += n;
        Some(s)
    };
    let head = take(4)?;
    let (cats, count) = (usize::from(u16::from_le_bytes([head[0], head[1]])), usize::from(u16::from_le_bytes([head[2], head[3]])));
    let mut categories = Vec::with_capacity(cats);
    for _ in 0..cats {
        let n = usize::from(take(1)?[0]);
        categories.push(std::str::from_utf8(take(n)?).ok()?);
    }
    let mut patches = Vec::with_capacity(count);
    for _ in 0..count {
        let h = take(2)?;
        let name = std::str::from_utf8(take(usize::from(h[1]))?).ok()?;
        let n = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
        patches.push(Patch { category: usize::from(h[0]), name, data: take(n)? });
    }
    Some(Bank { categories, patches })
}

pub fn bank() -> &'static Bank {
    static B: OnceLock<Bank> = OnceLock::new();
    B.get_or_init(|| read().unwrap_or(Bank { categories: Vec::new(), patches: Vec::new() }))
}
