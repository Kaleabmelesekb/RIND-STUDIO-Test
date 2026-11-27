//! Freezing a channel (a Honeydew feature): its sound rendered to an audio channel, and its own notes muted so that its
//! instrument stops working. The song holds no flag for it. The frozen audio is the Audio channel right after the
//! source named `<source name> (frozen)`, and the source's note and drum clips are muted: both are what a project file
//! already holds, so a frozen song opens in any version.

use super::{ChannelKind, LaneKind, Song};

/// What follows the source's name in the name of its frozen audio channel.
pub const FROZEN: &str = " (frozen)";
/// What follows the source's name in the name of a bounce's audio channel (`Bounce in Place`: nothing is muted).
pub const BOUNCE: &str = " (bounce)";

impl Song {
    /// The Audio channel that holds channel `index` frozen: the one right after it, named `<its name> (frozen)`.
    pub fn frozen_of(&self, index: usize) -> Option<usize> {
        let source = self.channels.get(index)?;
        let next = index.checked_add(1)?;
        let audio = self.channels.get(next)?;
        let name = format!("{}{FROZEN}", source.name);
        (audio.kind == ChannelKind::AudioClip && audio.name == name).then_some(next)
    }

    /// Whether channel `index` has anything to freeze: a note or hit that plays, in a clip that is not muted.
    pub fn has_notes(&self, index: usize) -> bool {
        let Some(channel) = self.channels.get(index) else {
            return false;
        };
        let plays = |kind: LaneKind| matches!(kind, LaneKind::Notes | LaneKind::Drums);
        (channel.lanes.iter())
            .filter(|l| plays(l.kind))
            .any(|l| l.clips.iter().any(|c| !c.muted && !c.events.is_empty()))
    }

    /// Mutes (`muted`) or unmutes every clip of the notes and drums tracks of channel `index`; its automation stays.
    /// Returns whether a clip changed.
    pub fn mute_note_clips(&mut self, index: usize, muted: bool) -> bool {
        let Some(channel) = self.channels.get_mut(index) else {
            return false;
        };
        let mut changed = false;
        for lane in &mut channel.lanes {
            if !matches!(lane.kind, LaneKind::Notes | LaneKind::Drums) {
                continue;
            }
            for clip in &mut lane.clips {
                changed |= clip.muted != muted;
                clip.muted = muted;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Event, SynthKind};

    fn song_with_notes() -> Song {
        let mut song = Song::default();
        song.add_channel(ChannelKind::Synth(SynthKind::MiniSynth));
        let lane = &mut song.channels[1].lanes[0];
        let clip = lane.add_clip(0, 4.0);
        clip.events.push(Event::Note {
            pos: 0,
            len_beats: 1.0,
            key: 60,
            velocity: 100,
            pan: 64,
            slide: false,
        });
        song
    }

    #[test]
    fn a_channel_with_a_note_has_notes_until_its_clips_are_muted() {
        let mut song = song_with_notes();
        assert!(song.has_notes(1));
        assert!(song.mute_note_clips(1, true));
        assert!(!song.has_notes(1));
        assert!(!song.mute_note_clips(1, true), "nothing left to change");
        assert!(song.mute_note_clips(1, false));
        assert!(song.has_notes(1));
    }

    #[test]
    fn the_frozen_audio_is_the_channel_after_named_for_its_source() {
        let mut song = song_with_notes();
        assert_eq!(song.frozen_of(1), None);
        let audio = song
            .insert_channel(2, ChannelKind::AudioClip)
            .expect("room for a channel");
        assert_eq!(song.frozen_of(1), None, "not named for it yet");
        song.channels[audio].name = format!("{}{FROZEN}", song.channels[1].name);
        assert_eq!(song.frozen_of(1), Some(audio));
        song.channels[audio].name = format!("{}{BOUNCE}", song.channels[1].name);
        assert_eq!(song.frozen_of(1), None, "a bounce is not a freeze");
        assert_eq!(song.frozen_of(9), None);
    }
}
