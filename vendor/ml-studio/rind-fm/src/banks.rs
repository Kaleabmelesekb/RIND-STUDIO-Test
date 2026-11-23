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

/// The voices of a sysex file, as banks of 32 at most. Real files are untidy, so this is forgiving: it looks for
/// DX7 messages anywhere in the file (a 32-voice dump `F0 43 0n 09 ..` or a single voice `F0 43 0n 00 ..`, whatever
/// their byte counts and checksums say), takes every one of them, and reads a file without any framing as raw
/// 128-byte voices.
pub fn parse_syx(bytes: &[u8], name: &str) -> Vec<Bank> {
    let mut voices: Vec<Voice> = Vec::new();
    let mut at = 0;
    let mut framed = false;
    while at + 6 <= bytes.len() {
        if bytes[at] == 0xF0 && bytes[at + 1] == 0x43 && bytes[at + 2] & 0xF0 == 0x00 {
            match bytes[at + 3] {
                // A bank of 32 packed voices.
                0x09 if at + 6 + 4096 <= bytes.len() => {
                    framed = true;
                    voices.extend((0..32).map(|i| unpack(&bytes[at + 6 + i * 128..])));
                    at += 6 + 4096;
                    continue;
                }
                // One voice, 155 bytes.
                0x00 if at + 6 + 155 <= bytes.len() => {
                    framed = true;
                    let mut v: Voice = [0; 156];
                    v[..155].copy_from_slice(&bytes[at + 6..at + 6 + 155]);
                    for b in &mut v[..145] {
                        *b &= 0x7f;
                    }
                    v[155] = 0x3f;
                    voices.push(v);
                    at += 6 + 155;
                    continue;
                }
                _ => {}
            }
        }
        at += 1;
    }
    if !framed && !bytes.is_empty() && bytes.len() % 128 == 0 && bytes.len() <= 4096 * 8 {
        voices.extend(bytes.chunks_exact(128).map(unpack));
    }
    let chunks = voices.chunks(32).count();
    voices
        .chunks(32)
        .enumerate()
        .map(|(i, v)| Bank { name: if chunks > 1 { format!("{name} {}", i + 1) } else { name.to_string() }, voices: v.to_vec() })
        .collect()
}
