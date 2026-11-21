//! Edits of a note clip's notes straight from the playlist's `More...` menu, without opening the piano roll: the
//! piano roll's own note edits ([`crate::ui::piano_roll::notes`]) applied to every note of the clip.
//!
//! Only notes lanes take them (a drum lane's keys are pads, not pitches). An edit that would leave a note past the
//! clip's loop (a quantize that rounds the last start up to the loop's end) changes nothing.

use crate::song::{Clip, Event, PPQ};
use crate::ui::piano_roll::notes::{
    legato_selected, note, quantize_selected, reverse_notes, shift_notes, sort,
};

/// What the menu asks of a note clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteOp {
    /// Every note this many semitones up (negative: down), stopped at the ends of the key range together.
    Transpose(i32),
    /// Mirror the notes in time inside their own span.
    Reverse,
    /// Lengthen each note to the next one's start.
    Legato,
    /// Move every start to the nearest sixteenth.
    Quantize,
}

/// The menu's items, in order.
pub const NOTE_OPS: [(NoteOp, &str); 7] = [
    (NoteOp::Transpose(1), "Semitone up"),
    (NoteOp::Transpose(-1), "Semitone down"),
    (NoteOp::Transpose(12), "Octave up"),
    (NoteOp::Transpose(-12), "Octave down"),
    (NoteOp::Reverse, "Reverse notes"),
    (NoteOp::Legato, "Legato"),
    (NoteOp::Quantize, "Quantize 1/16"),
];

/// The clip holds a note an edit can act on.
pub fn has_notes(clip: &Clip) -> bool {
    clip.events.iter().any(|e| note(e).is_some())
}

/// Applies `op` to every note of `clip`; `true` when anything changed.
pub fn apply(clip: &mut Clip, op: NoteOp) -> bool {
    let mut sel: Vec<bool> = clip.events.iter().map(|e| note(e).is_some()).collect();
    if !sel.contains(&true) {
        return false;
    }
    let before: Vec<Event> = clip.events.clone();
    match op {
        NoteOp::Transpose(dk) => clip.events = shift_notes(&before, &sel, 0.0, dk),
        NoteOp::Reverse => reverse_notes(&mut clip.events, &sel),
        NoteOp::Legato => legato_selected(&mut clip.events, &sel, f64::from(PPQ) * 4.0),
        NoteOp::Quantize => {
            quantize_selected(&mut clip.events, &mut sel, f64::from(PPQ) / 4.0);
        }
    }
    sort(&mut clip.events, &mut sel);
    let past = clip.loop_ticks().is_some_and(|lp| {
        clip.events
            .iter()
            .any(|e| note(e).is_some_and(|(pos, ..)| pos >= lp))
    });
    if past || clip.events == before {
        clip.events = before;
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Lane, LaneKind};

    fn note_at(pos: u32, len_beats: f64, key: u8) -> Event {
        Event::Note {
            pos,
            len_beats,
            key,
            velocity: Event::DEFAULT_VELOCITY,
            pan: Event::DEFAULT_PAN,
            slide: false,
        }
    }

    fn clip_with(events: &[Event]) -> Clip {
        let mut lane = Lane::new(LaneKind::Notes);
        let clip = lane.add_clip(0, 4.0);
        clip.events = events.to_vec();
        clip.clone()
    }

    fn keys(clip: &Clip) -> Vec<u8> {
        clip.events
            .iter()
            .filter_map(|e| note(e).map(|(_, _, k)| k))
            .collect()
    }

    fn starts(clip: &Clip) -> Vec<u32> {
        clip.events.iter().map(Event::pos).collect()
    }

    #[test]
    fn transposing_moves_every_note_together_and_stops_at_the_top() {
        let mut c = clip_with(&[note_at(0, 0.5, 60), note_at(128, 0.5, 64)]);
        assert!(apply(&mut c, NoteOp::Transpose(12)));
        assert_eq!(keys(&c), [72, 76]);
        assert!(apply(&mut c, NoteOp::Transpose(-1)));
        assert_eq!(keys(&c), [71, 75]);
        let mut high = clip_with(&[note_at(0, 0.5, 120), note_at(128, 0.5, 127)]);
        // The highest note is at the top: nothing moves, the chord keeps its shape.
        assert!(!apply(&mut high, NoteOp::Transpose(1)));
        assert_eq!(keys(&high), [120, 127]);
    }

    #[test]
    fn reverse_mirrors_the_starts_inside_the_span_and_keeps_the_order_sorted() {
        let mut c = clip_with(&[
            note_at(0, 0.25, 60),
            note_at(128, 0.25, 62),
            note_at(256, 0.25, 64),
        ]);
        assert!(apply(&mut c, NoteOp::Reverse));
        assert_eq!(keys(&c), [64, 62, 60]);
        let s = starts(&c);
        assert!(s.windows(2).all(|w| w[0] <= w[1]), "{s:?}");
    }

    #[test]
    fn legato_joins_each_note_to_the_next_and_the_last_to_the_bar_end() {
        let mut c = clip_with(&[note_at(0, 0.25, 60), note_at(128, 0.25, 62)]);
        assert!(apply(&mut c, NoteOp::Legato));
        let Event::Note { len_beats, .. } = c.events[0] else {
            panic!()
        };
        assert!((len_beats - 1.0).abs() < 1e-9, "{len_beats}");
        // The last note reaches the end of its bar (#123).
        let Event::Note {
            len_beats: last, ..
        } = c.events[1]
        else {
            panic!()
        };
        assert!(last > 0.25, "{last}");
    }

    #[test]
    fn quantize_rounds_starts_to_sixteenths_and_never_pushes_a_note_past_the_loop() {
        let mut c = clip_with(&[note_at(5, 0.25, 60), note_at(50, 0.25, 62)]);
        assert!(apply(&mut c, NoteOp::Quantize));
        assert_eq!(starts(&c), [0, 64]);
        assert!(!apply(&mut c, NoteOp::Quantize), "already on the grid");
        // A start 4 ticks before the loop end would round up to it: refused as a whole.
        let end = 4 * PPQ;
        let mut edge = clip_with(&[note_at(end - 4, 0.25, 60)]);
        assert!(!apply(&mut edge, NoteOp::Quantize));
        assert_eq!(starts(&edge), [end - 4]);
    }

    #[test]
    fn a_clip_without_notes_takes_no_edit() {
        let mut c = clip_with(&[]);
        assert!(!has_notes(&c));
        for (op, _) in NOTE_OPS {
            assert!(!apply(&mut c, op));
        }
    }
}
