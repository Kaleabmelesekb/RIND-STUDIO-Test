// SPDX-License-Identifier: GPL-3.0-or-later
//! The voice banks: the 33 Dexed ships (in the app) and the cartridges (`.syx`) a user loads.
use std::os::raw::c_char;

/// A voice as the engine plays it: 155 bytes of a DX7 voice (the last is the operator switches).
pub type Voice = [u8; 156];

/// The built-in banks' voices, 32 of 128 packed bytes each (sysex banks without their frame).
static BANKS: &[u8] = include_bytes!("../banks/banks.bin");
static BANK_NAMES: &str = include_str!("../banks/names.txt");
const BANK_LEN: usize = 32 * 128;

pub fn builtin() -> usize {
    BANKS.len() / BANK_LEN
}

pub fn builtin_name(bank: usize) -> &'static str {
    BANK_NAMES.lines().nth(bank).unwrap_or("")
}

/// A voice made from the 128 bytes a bank stores.
pub fn unpack(packed: &[u8]) -> Voice {
    let mut out: Voice = [0; 156];
    if packed.len() >= 128 {
        let src: Vec<c_char> = packed[..128].iter().map(|&b| b as c_char).collect();
        let mut dst = [0 as c_char; 156];
        unsafe { crate::rdx_unpack(src.as_ptr(), dst.as_mut_ptr()) };
        for (o, d) in out.iter_mut().zip(dst) {
            *o = d as u8;
        }
    }
    out
}

pub fn builtin_voice(bank: usize, voice: usize) -> Voice {
    let at = bank.min(builtin().saturating_sub(1)) * BANK_LEN + voice.min(31) * 128;
    unpack(&BANKS[at..at + 128])
}

/// A voice's name: its ten characters, tidied.
pub fn name_of(v: &Voice) -> String {
    let s: String = v[145..155].iter().map(|&b| if (32..127).contains(&b) { b as char } else { ' ' }).collect();
    s.trim().to_string()
}

/// A cartridge the user loaded.
pub struct Bank {
    pub name: String,
    pub voices: Vec<Voice>,
}

/// The voices of a sysex file: a 32-voice bank (with or without its frame) or one voice.
pub fn parse_syx(bytes: &[u8], name: &str) -> Option<Bank> {
    let framed = |head: &[u8], len: usize| bytes.len() >= len && bytes[0] == 0xF0 && bytes[1] == 0x43 && bytes[3..6] == *head;
    let voices: Vec<Voice> = if framed(&[0x09, 0x20, 0x00], 6 + 4096) {
        (0..32).map(|i| unpack(&bytes[6 + i * 128..])).collect()
    } else if bytes.len() == 4096 {
        (0..32).map(|i| unpack(&bytes[i * 128..])).collect()
    } else if framed(&[0x00, 0x01, 0x1B], 6 + 155) {
        let mut v: Voice = [0; 156];
        v[..155].copy_from_slice(&bytes[6..6 + 155]);
        v[155] = 0x3f;
        vec![v]
    } else {
        return None;
    };
    Some(Bank { name: name.to_string(), voices })
}
