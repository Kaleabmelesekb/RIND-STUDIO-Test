//! `Freeze Track` and `Bounce in Place` (Honeydew's own): a channel rendered on its own, as a stem is, behind the busy
//! screen ([`Job::bounce`]); the WAV kept in the user's `My Recordings`, then a new Audio channel inserted right after
//! the channel, as a dropped file makes one ([`super::file_drop`]), named after it, of its colour and output.
//!
//! A freeze also mutes the channel's note and drum clips in the same undo step, so its instrument has nothing to play and
//! stops costing CPU; `Unfreeze Track` deletes the Audio channel and unmutes them ([`Song::frozen_of`]). A bounce leaves
//! the channel as it is.

use std::path::PathBuf;

use super::HoneydewApp;
use super::file_drop::Bounce;
use crate::project_files::export::{Job, NO_AUDIO_RATE};
use crate::sample_cache;
use crate::song::freeze::{BOUNCE, FROZEN};
use crate::ui::main_menu::library::FORBIDDEN;
use crate::ui::views::{Nav, Overlay};

/// The message of a channel with no notes to render.
const NOTHING: &str = "Nothing to render: the channel has no notes";

impl HoneydewApp {
    /// Starts rendering channel `channel` for a freeze (`freeze`) or a bounce. The song is not touched until the audio is
    /// in ([`Self::bounced`]).
    pub(super) fn start_bounce(&mut self, channel: usize, freeze: bool) {
        if self.export.is_some() || self.importing.is_some() || self.bounce.is_some() {
            return;
        }
        if !self.song.has_notes(channel) {
            self.files.flash(NOTHING);
            return;
        }
        let Some(name) = self.song.channels.get(channel).map(|c| c.name.clone()) else {
            return;
        };
        let id = self.song.channels[channel].id;
        self.audio
            .samples
            .load(&self.song, &sample_cache::reader(&self.files));
        let rate = match self.song.sample_rate {
            0 => self.audio.sample_rate().unwrap_or(NO_AUDIO_RATE),
            rate => rate,
        };
        let verb = if freeze { "Freezing" } else { "Bouncing" };
        let label = format!("{verb} {name}");
        match Job::bounce(&self.song, &self.audio.samples, channel, rate, label) {
            Ok(job) => {
                self.main_screen.playing = false;
                self.navigate(Nav::Open(Overlay::Busy { title: job.title() }));
                self.export = Some(job);
                self.bounce = Some(Bounce {
                    channel: id,
                    freeze,
                });
            }
            Err(error) => self.files.render_failed(&error),
        }
    }

    /// The bounce's render is done, `bytes` its WAV file: kept in `My Recordings`, then decoded for its new channel.
    pub(super) fn bounced(&mut self, bytes: Vec<u8>, frame: u64) {
        let Some(bounce) = self.bounce.take() else {
            return;
        };
        let Some(source) = self
            .song
            .channels
            .iter()
            .position(|c| c.id == bounce.channel)
        else {
            return;
        };
        let suffix = if bounce.freeze { FROZEN } else { BOUNCE };
        let name = self.song.channels[source]
            .name
            .replace(|c: char| FORBIDDEN.contains(c) || c.is_control(), "_");
        let file = PathBuf::from(format!("{name}{suffix}.wav"));
        match self.files.keep_saved(&file, bytes) {
            Some(kept) => self.import_bounce(kept, source + 1, bounce, frame),
            None => self.files.flash("The audio could not be saved"),
        }
    }

    /// `Unfreeze Track` on channel `channel`: its frozen Audio channel deleted and its notes unmuted, one undo step.
    pub(super) fn unfreeze(&mut self, channel: usize, tick: u64) {
        let Some(frozen) = self.song.frozen_of(channel) else {
            return;
        };
        self.song.mute_note_clips(channel, false);
        self.delete_channel(frozen, tick);
    }
}
