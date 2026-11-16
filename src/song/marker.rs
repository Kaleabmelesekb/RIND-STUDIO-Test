//! Markers (Ableton's locators): named points on the song's timeline that the control bar jumps between and the
//! playlist's ruler shows as flags.
//!
//! The song keeps them sorted by tick with at most one at a tick ([`Song::markers`]). The project file holds them in
//! one `RMRK` chunk after the track groups, written only while the song has a marker (a song without one is byte for
//! byte what it was) and read back by [`decode`]; a reader that does not know the chunk keeps it as an unknown one.

use super::{PPQ, Song};

/// The most markers a song keeps, and the longest marker name, in characters.
pub const MAX_MARKERS: usize = 512;
pub const MAX_NAME: usize = 32;

/// The `RMRK` chunk's version byte.
const VERSION: u8 = 1;

/// A jump to the previous marker from less than this far past one goes to the one before it (as in every DAW, so a
/// second press while the playhead sits just after a marker keeps going back).
pub const BACK_SLACK: u32 = PPQ / 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// The song tick the marker sits on.
    pub tick: u32,
    pub name: String,
}

/// The index of the marker at `tick`.
pub fn index_at(markers: &[Marker], tick: u32) -> Option<usize> {
    markers.binary_search_by_key(&tick, |m| m.tick).ok()
}

/// The first unused `Marker n` name, counting from 1.
pub fn next_name(markers: &[Marker]) -> String {
    let used = |n: usize| markers.iter().any(|m| m.name == format!("Marker {n}"));
    let n = (1..).find(|&n| !used(n)).unwrap_or(1);
    format!("Marker {n}")
}

/// Puts a marker at `tick` if there is none, else takes the one there away; `true` when one was added. A full song
/// (`MAX_MARKERS`) adds none.
pub fn toggle(markers: &mut Vec<Marker>, tick: u32) -> bool {
    match markers.binary_search_by_key(&tick, |m| m.tick) {
        Ok(i) => {
            markers.remove(i);
            false
        }
        Err(i) if markers.len() < MAX_MARKERS => {
            let name = next_name(markers);
            markers.insert(i, Marker { tick, name });
            true
        }
        Err(_) => false,
    }
}

/// The tick a jump back from `tick` goes to: the last marker at least [`BACK_SLACK`] ticks before it.
pub fn before(markers: &[Marker], tick: u64) -> Option<u32> {
    (markers.iter().rev())
        .map(|m| m.tick)
        .find(|&t| u64::from(t) + u64::from(BACK_SLACK) <= tick)
}

/// The tick a jump forward from `tick` goes to: the first marker after it.
pub fn after(markers: &[Marker], tick: u64) -> Option<u32> {
    markers
        .iter()
        .map(|m| m.tick)
        .find(|&t| u64::from(t) > tick)
}

/// The marker whose flag the playhead is on or past last, for the control bar's readout.
pub fn current(markers: &[Marker], tick: u64) -> Option<&Marker> {
    markers.iter().rev().find(|m| u64::from(m.tick) <= tick)
}

impl Song {
    /// Puts a marker at `tick`, or takes the one there; `true` when one was added.
    pub fn toggle_marker(&mut self, tick: u32) -> bool {
        toggle(&mut self.markers, tick)
    }
}

/// The `RMRK` chunk's payload: empty for no markers.
pub fn encode(markers: &[Marker]) -> Vec<u8> {
    let kept = &markers[..markers.len().min(MAX_MARKERS)];
    if kept.is_empty() {
        return Vec::new();
    }
    let mut out = vec![VERSION];
    out.extend((kept.len() as u16).to_le_bytes());
    for m in kept {
        let name: String = m.name.chars().take(MAX_NAME).collect();
        out.extend(m.tick.to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(name.as_bytes());
    }
    out
}

/// The markers in an `RMRK` payload, sorted and one to a tick; `None` for a payload that is not one.
pub fn decode(bytes: &[u8]) -> Option<Vec<Marker>> {
    let mut at: usize = 0;
    let mut take = |n: usize| {
        let part = bytes.get(at..at.checked_add(n)?)?;
        at += n;
        Some(part)
    };
    if *take(1)?.first()? != VERSION {
        return None;
    }
    let count = u16::from_le_bytes(take(2)?.try_into().ok()?);
    let mut markers = Vec::new();
    for _ in 0..count.min(MAX_MARKERS as u16) {
        let tick = u32::from_le_bytes(take(4)?.try_into().ok()?);
        let len = usize::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
        let name: String = String::from_utf8_lossy(take(len)?)
            .chars()
            .take(MAX_NAME)
            .collect();
        markers.push(Marker { tick, name });
    }
    markers.sort_by_key(|m| m.tick);
    markers.dedup_by_key(|m| m.tick);
    Some(markers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ticks: &[u32]) -> Vec<Marker> {
        let mut v = Vec::new();
        for &t in ticks {
            toggle(&mut v, t);
        }
        v
    }

    #[test]
    fn toggling_adds_in_order_and_removes_the_one_there() {
        let mut v = at(&[512, 0, 256]);
        assert_eq!(v.iter().map(|m| m.tick).collect::<Vec<_>>(), [0, 256, 512]);
        assert!(!toggle(&mut v, 256));
        assert_eq!(v.iter().map(|m| m.tick).collect::<Vec<_>>(), [0, 512]);
        assert!(toggle(&mut v, 256));
        assert_eq!(index_at(&v, 256), Some(1));
        assert_eq!(index_at(&v, 257), None);
    }

    #[test]
    fn names_count_up_and_fill_the_gaps() {
        let mut v = at(&[0, 128, 256]);
        assert_eq!(
            v.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["Marker 1", "Marker 2", "Marker 3"]
        );
        toggle(&mut v, 128);
        toggle(&mut v, 64);
        assert_eq!(v[1].tick, 64);
        assert_eq!(v[1].name, "Marker 2");
    }

    #[test]
    fn a_full_song_adds_no_more() {
        let mut v = Vec::new();
        for t in 0..MAX_MARKERS as u32 + 5 {
            toggle(&mut v, t);
        }
        assert_eq!(v.len(), MAX_MARKERS);
        assert!(!toggle(&mut v, 100_000));
        assert_eq!(v.len(), MAX_MARKERS);
    }

    #[test]
    fn jumps_go_to_the_neighbours_and_back_skips_a_marker_just_passed() {
        let v = at(&[0, 512, 1024]);
        assert_eq!(after(&v, 0), Some(512));
        assert_eq!(after(&v, 511), Some(512));
        assert_eq!(after(&v, 1024), None);
        assert_eq!(before(&v, 1024), Some(512));
        assert_eq!(before(&v, 1100), Some(1024));
        // Just past a marker, back goes to the one before it.
        assert_eq!(before(&v, 1024 + u64::from(BACK_SLACK) - 1), Some(512));
        assert_eq!(before(&v, 1024 + u64::from(BACK_SLACK)), Some(1024));
        assert_eq!(before(&v, 0), None);
        assert_eq!(before(&[], 999), None);
        assert_eq!(after(&[], 0), None);
    }

    #[test]
    fn the_current_marker_is_the_last_one_passed() {
        let v = at(&[128, 512]);
        assert!(current(&v, 100).is_none());
        assert_eq!(current(&v, 128).map(|m| m.tick), Some(128));
        assert_eq!(current(&v, 600).map(|m| m.tick), Some(512));
    }

    #[test]
    fn the_chunk_round_trips_and_a_song_without_markers_writes_nothing() {
        assert!(encode(&[]).is_empty());
        let mut v = at(&[10, 20, 30]);
        v[1].name = "Drop ✦".into();
        assert_eq!(decode(&encode(&v)), Some(v));
    }

    #[test]
    fn a_damaged_chunk_is_refused_and_a_messy_one_is_tidied() {
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(&[9, 0, 0]), None);
        let good = encode(&at(&[1, 2]));
        assert_eq!(decode(&good[..good.len() - 1]), None);
        // Out of order and doubled ticks come back sorted, one to a tick.
        let mut bytes = vec![VERSION, 3, 0];
        for t in [50u32, 10, 50] {
            bytes.extend(t.to_le_bytes());
            bytes.extend(0u16.to_le_bytes());
        }
        let v = decode(&bytes).unwrap();
        assert_eq!(v.iter().map(|m| m.tick).collect::<Vec<_>>(), [10, 50]);
    }

    #[test]
    fn long_names_are_cut_when_written() {
        let mut v = at(&[0]);
        v[0].name = "x".repeat(100);
        let back = decode(&encode(&v)).unwrap();
        assert_eq!(back[0].name.chars().count(), MAX_NAME);
    }
}
