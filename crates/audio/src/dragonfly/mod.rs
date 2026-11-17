//! Dragonfly Reverb's engines, ported to Rust: Hall, Room, Plate and Early Reflections.
//!
//! Dragonfly Reverb is by Michael Willis and Rob van den Berg (GPL-3.0-or-later) and rests on Freeverb3 by Teru
//! Kamogashira (GPL-2.0-or-later); the Hall's late field follows Fons Adriaensen's zita-rev1. The code here is a
//! port of their DSP, with buffers sized up front so that the audio thread never allocates.
// The ports index several parallel tables by one stage number, as the originals do.
#![allow(clippy::needless_range_loop)]

pub mod early;
pub mod early_taps;
pub mod hall;
pub mod noise;
pub mod plate;
pub mod prims;
pub mod room;
