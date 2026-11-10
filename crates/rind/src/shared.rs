// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the sound shares with the editor without a lock: the output meters, the voice count and the on-screen
//! keyboard's note events. Everything is an atomic; the audio thread never waits.

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

const QUEUE: usize = 64;

pub struct Shared {
    peak: [AtomicU32; 2],
    voices: AtomicU32,
    /// Note events from the on-screen keyboard: `note | 0x80` is an on, a plain `note` an off; 0xFFFF is empty.
    queue: [AtomicU32; QUEUE],
    head: AtomicU32,
    tail: AtomicU32,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            peak: [AtomicU32::new(0), AtomicU32::new(0)],
            voices: AtomicU32::new(0),
            queue: std::array::from_fn(|_| AtomicU32::new(0)),
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
        }
    }
}

impl Shared {
    /// The audio thread's peaks of a block; the meter falls by `fall` per block (a fraction kept).
    pub fn set_levels(&self, l: f32, r: f32, fall: f32) {
        for (slot, now) in self.peak.iter().zip([l, r]) {
            let before = f32::from_bits(slot.load(Relaxed));
            slot.store(now.max(before * fall).to_bits(), Relaxed);
        }
    }

    /// The meters' linear levels (left, right).
    pub fn levels(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak[0].load(Relaxed)),
            f32::from_bits(self.peak[1].load(Relaxed)),
        )
    }

    pub fn set_voices(&self, n: usize) {
        self.voices.store(n as u32, Relaxed);
    }

    pub fn voices(&self) -> usize {
        self.voices.load(Relaxed) as usize
    }

    /// The editor's keyboard: a note on or off. False when the queue is full.
    pub fn push_key(&self, note: u8, on: bool) -> bool {
        let (head, tail) = (self.head.load(Relaxed), self.tail.load(Relaxed));
        if head.wrapping_sub(tail) as usize >= QUEUE {
            return false;
        }
        let v = (note & 0x7f) as u32 | if on { 0x80 } else { 0 };
        self.queue[head as usize % QUEUE].store(v, Relaxed);
        self.head
            .store(head.wrapping_add(1), std::sync::atomic::Ordering::Release);
        true
    }

    /// The audio thread's next keyboard event: (note, on).
    pub fn pop_key(&self) -> Option<(u8, bool)> {
        let tail = self.tail.load(Relaxed);
        if tail == self.head.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        let v = self.queue[tail as usize % QUEUE].load(Relaxed);
        self.tail.store(tail.wrapping_add(1), Relaxed);
        Some(((v & 0x7f) as u8, v & 0x80 != 0))
    }
}

/// A linear level on the meter's dB scale: how many of its ten lamps (-24 .. +3 dB) light, 0 to 10.
pub fn lamps(level: f32) -> usize {
    const EDGES: [f32; 10] = [
        -24.0, -21.0, -18.0, -15.0, -12.0, -8.0, -6.0, -3.0, 0.0, 3.0,
    ];
    let db = 20.0 * level.max(1e-6).log10();
    EDGES.iter().filter(|&&e| db >= e).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_come_out_in_order_and_the_queue_bounds() {
        let s = Shared::default();
        assert_eq!(s.pop_key(), None);
        assert!(s.push_key(60, true));
        assert!(s.push_key(60, false));
        assert_eq!(s.pop_key(), Some((60, true)));
        assert_eq!(s.pop_key(), Some((60, false)));
        assert_eq!(s.pop_key(), None);
        for i in 0..QUEUE {
            assert!(s.push_key(i as u8, true));
        }
        assert!(!s.push_key(1, true));
        assert_eq!(s.pop_key(), Some((0, true)));
        assert!(s.push_key(1, true));
    }

    #[test]
    fn the_meter_rises_at_once_and_falls_slowly() {
        let s = Shared::default();
        s.set_levels(0.5, 0.0, 0.8);
        assert_eq!(s.levels(), (0.5, 0.0));
        s.set_levels(0.0, 0.0, 0.8);
        assert!((s.levels().0 - 0.4).abs() < 1e-6);
    }

    #[test]
    fn lamps_follow_the_db_scale() {
        assert_eq!(lamps(0.0), 0);
        assert_eq!(lamps(0.6), 7); // -4.4 dB
        assert_eq!(lamps(0.95), 8); // -0.4 dB: the red lamp waits for full scale
        assert_eq!(lamps(1.0), 9); // 0 dB
        assert_eq!(lamps(2.0), 10); // +6 dB
    }
}
