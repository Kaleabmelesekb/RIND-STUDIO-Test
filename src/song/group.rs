//! Track groups, as in Ableton and Bitwig: a named, coloured set of channels the playlist can fold into one row.
//!
//! A group is a song-level list of channel ids ([`TrackGroup`]); a channel is in at most one. A folded group shows its
//! first channel's row alone (the group's head) and hides the rest, so a project of many tracks reads as a few. The
//! channels keep their own racks, routing and mixer state: grouping changes what the playlist shows, not what plays.
//!
//! The project file holds the groups in one `RGRP` chunk after the channels, written only while the song has a group
//! (a song without one is byte for byte what it was) and read back by [`decode`]; a reader that does not know the
//! chunk keeps it as an unknown one.

use super::Song;

/// The most groups a song keeps, and the longest group name, in characters.
pub const MAX_GROUPS: usize = 256;
pub const MAX_NAME: usize = 48;

/// The `RGRP` chunk's version byte.
const VERSION: u8 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct TrackGroup {
    /// Unique in the song, never reused while the group lives.
    pub id: i32,
    pub name: String,
    /// The group's colour, a channel hue (see `theme::channel_color`).
    pub hue: f32,
    /// The playlist shows the group's head row alone.
    pub collapsed: bool,
    /// The channels in it, by [`super::Channel::id`].
    pub members: Vec<i32>,
}

impl Song {
    /// The group with id `id`.
    pub fn group(&self, id: i32) -> Option<&TrackGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// The group channel `id` (a channel id) is in, if any of its members still exist.
    pub fn group_of(&self, id: i32) -> Option<&TrackGroup> {
        self.groups.iter().find(|g| g.members.contains(&id))
    }

    /// The members of `group` that are channels of the song, as indices in playlist order. The master is never one.
    pub fn group_channels(&self, group: &TrackGroup) -> Vec<usize> {
        (self.playlist().into_iter())
            .filter(|&c| c > 0 && group.members.contains(&self.channels[c].id))
            .collect()
    }

    /// The channel (index) heading `group`: its first member in the playlist.
    pub fn group_head(&self, group: &TrackGroup) -> Option<usize> {
        self.group_channels(group).first().copied()
    }

    /// Groups channel `channel` (an index) with the channel above it in the playlist: it joins that channel's group,
    /// or the two make a new one. Neither is the master, and the channel moves under the group's last member so the
    /// group stays together. False, and no change, for the master, a first row, or a channel already in that group.
    pub fn group_with_above(&mut self, channel: usize) -> bool {
        let order = self.playlist();
        let Some(place) = order.iter().position(|&c| c == channel) else {
            return false;
        };
        let above = place.checked_sub(1).map(|p| order[p]).filter(|&c| c > 0);
        let (Some(above), true) = (above, channel > 0) else {
            return false;
        };
        let (above_id, id) = (self.channels[above].id, self.channels[channel].id);
        let target = match self.group_of(above_id) {
            Some(g) if g.members.contains(&id) => return false,
            Some(g) => g.id,
            None => {
                if self.groups.len() >= MAX_GROUPS {
                    return false;
                }
                let gid = self.next_group_id();
                let name = self.free_group_name();
                let hue = self.channels[above].hue;
                self.groups.push(TrackGroup {
                    id: gid,
                    name,
                    hue,
                    collapsed: false,
                    members: vec![above_id],
                });
                gid
            }
        };
        self.leave_group(channel);
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == target) {
            g.members.push(id);
        }
        self.gather_group(target);
        true
    }

    /// Takes channel `channel` (an index) out of its group; a group left with one member or none is dissolved.
    /// False, and no change, for a channel in no group.
    pub fn leave_group(&mut self, channel: usize) -> bool {
        let Some(id) = self.channels.get(channel).map(|c| c.id) else {
            return false;
        };
        let Some(at) = self.groups.iter().position(|g| g.members.contains(&id)) else {
            return false;
        };
        self.groups[at].members.retain(|&m| m != id);
        if self.groups[at].members.len() < 2 {
            self.groups.remove(at);
        }
        true
    }

    /// Dissolves the group `id`, its channels staying where they are.
    pub fn ungroup(&mut self, id: i32) -> bool {
        let before = self.groups.len();
        self.groups.retain(|g| g.id != id);
        self.groups.len() != before
    }

    /// Folds (`true`) or unfolds the group `id`. False, and no change, for no such group or when it is already so.
    pub fn set_group_collapsed(&mut self, id: i32, collapsed: bool) -> bool {
        match self.groups.iter_mut().find(|g| g.id == id) {
            Some(g) if g.collapsed != collapsed => {
                g.collapsed = collapsed;
                true
            }
            _ => false,
        }
    }

    /// Moves the group's members next to each other in the playlist, at its head's place, in their playlist order.
    fn gather_group(&mut self, id: i32) {
        let Some(group) = self.group(id).cloned() else {
            return;
        };
        let members = self.group_channels(&group);
        let Some(&head) = members.first() else {
            return;
        };
        let order = self.playlist();
        let mut gathered: Vec<usize> = Vec::with_capacity(order.len());
        for &c in &order {
            if c == head {
                gathered.extend(&members);
            } else if !members.contains(&c) {
                gathered.push(c);
            }
        }
        self.set_playlist(&gathered);
    }

    /// The smallest unused group id.
    fn next_group_id(&self) -> i32 {
        (1..).find(|&id| self.group(id).is_none()).unwrap_or(1)
    }

    /// `Group`, else `Group N` for the smallest free `N >= 2`.
    fn free_group_name(&self) -> String {
        let taken = |n: &str| self.groups.iter().any(|g| g.name == n);
        if !taken("Group") {
            return "Group".to_owned();
        }
        (2..)
            .map(|n| format!("Group {n}"))
            .find(|n| !taken(n))
            .unwrap_or_default()
    }
}

/// The `RGRP` payload for `groups`, the members that no longer exist (`exists`) left out and a group with no member
/// left dropped; empty when nothing is left to write.
pub fn encode(groups: &[TrackGroup], exists: impl Fn(i32) -> bool) -> Vec<u8> {
    let kept: Vec<(&TrackGroup, Vec<i32>)> = (groups.iter().take(MAX_GROUPS))
        .map(|g| {
            (
                g,
                g.members
                    .iter()
                    .copied()
                    .filter(|&m| exists(m))
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(_, members)| !members.is_empty())
        .collect();
    if kept.is_empty() {
        return Vec::new();
    }
    let mut out = vec![VERSION];
    out.extend((kept.len() as u16).to_le_bytes());
    for (g, members) in kept {
        let name: String = g.name.chars().take(MAX_NAME).collect();
        out.extend(g.id.to_le_bytes());
        out.extend(g.hue.to_le_bytes());
        out.push(u8::from(g.collapsed));
        out.extend((name.len() as u16).to_le_bytes());
        out.extend(name.as_bytes());
        out.extend((members.len().min(usize::from(u16::MAX)) as u16).to_le_bytes());
        for m in members.iter().take(usize::from(u16::MAX)) {
            out.extend(m.to_le_bytes());
        }
    }
    out
}

/// The groups an `RGRP` payload holds; `None` for a version this reader does not know or a payload cut short.
pub fn decode(bytes: &[u8]) -> Option<Vec<TrackGroup>> {
    let mut at = 0;
    let mut take = |n: usize| {
        let part = bytes.get(at..at.checked_add(n)?)?;
        at += n;
        Some(part)
    };
    if *take(1)?.first()? != VERSION {
        return None;
    }
    let count = u16::from_le_bytes(take(2)?.try_into().ok()?);
    let mut groups = Vec::new();
    for _ in 0..count.min(MAX_GROUPS as u16) {
        let id = i32::from_le_bytes(take(4)?.try_into().ok()?);
        let hue = f32::from_le_bytes(take(4)?.try_into().ok()?);
        let collapsed = *take(1)?.first()? != 0;
        let len = usize::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
        let name: String = String::from_utf8_lossy(take(len)?)
            .chars()
            .take(MAX_NAME)
            .collect();
        let n = usize::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
        let mut members = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            members.push(i32::from_le_bytes(take(4)?.try_into().ok()?));
        }
        let hue = if hue.is_finite() {
            hue.clamp(0.0, 1.0)
        } else {
            0.0
        };
        groups.push(TrackGroup {
            id,
            name,
            hue,
            collapsed,
            members,
        });
    }
    Some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::ChannelKind;

    /// A song of the master and four instrument channels, ids 1 to 4 in playlist order.
    fn song() -> Song {
        let mut song = Song::default();
        for _ in 0..4 {
            song.add_channel(ChannelKind::Instrument);
        }
        song
    }

    fn ids(song: &Song) -> Vec<i32> {
        (song.playlist().into_iter())
            .map(|c| song.channels[c].id)
            .collect()
    }

    #[test]
    fn grouping_with_the_channel_above_makes_a_group_of_the_two_and_the_next_joins_it() {
        let mut song = song();
        assert!(song.group_with_above(2));
        let g = song.group_of(song.channels[1].id).unwrap().clone();
        assert_eq!(g.members, [song.channels[1].id, song.channels[2].id]);
        assert_eq!(g.name, "Group");
        assert!(song.group_with_above(3));
        assert_eq!(song.groups.len(), 1);
        assert_eq!(song.groups[0].members.len(), 3);
        // Already in it.
        assert!(!song.group_with_above(3));
    }

    #[test]
    fn the_master_and_the_first_row_do_not_group() {
        let mut song = song();
        assert!(!song.group_with_above(0));
        assert!(!song.group_with_above(1));
        assert!(song.groups.is_empty());
    }

    #[test]
    fn a_channel_joins_the_group_above_and_moves_next_to_it() {
        let mut song = song();
        // Channels 1 and 2 grouped; 3 is below, then 4. Group 4 with 3 (channel ids), then 3 with the group.
        assert!(song.group_with_above(2));
        let before = ids(&song);
        assert!(song.group_with_above(3));
        assert_eq!(ids(&song), before, "already together: no move");
        // The master sits first; the group's three channels are next to each other.
        let order = song.playlist();
        let heads: Vec<usize> = order.iter().copied().filter(|&c| c > 0).collect();
        assert_eq!(heads, [1, 2, 3, 4]);
    }

    #[test]
    fn leaving_a_group_of_two_dissolves_it() {
        let mut song = song();
        assert!(song.group_with_above(2));
        assert!(song.leave_group(2));
        assert!(song.groups.is_empty());
        assert!(!song.leave_group(2));
    }

    #[test]
    fn folding_hides_nothing_in_the_model_but_flags_the_group() {
        let mut song = song();
        song.group_with_above(2);
        let id = song.groups[0].id;
        assert!(song.set_group_collapsed(id, true));
        assert!(!song.set_group_collapsed(id, true));
        let g = song.group(id).unwrap().clone();
        assert_eq!(song.group_head(&g), Some(1));
        assert_eq!(song.group_channels(&g), [1, 2]);
        assert!(song.ungroup(id));
        assert!(!song.ungroup(id));
    }

    #[test]
    fn the_chunk_round_trips_and_leaves_out_what_is_gone() {
        let groups = vec![TrackGroup {
            id: 3,
            name: "Drums".into(),
            hue: 0.25,
            collapsed: true,
            members: vec![1, 2, 9],
        }];
        let bytes = encode(&groups, |id| id != 9);
        let back = decode(&bytes).unwrap();
        assert_eq!(back[0].members, [1, 2]);
        assert_eq!(
            (back[0].id, back[0].name.as_str(), back[0].collapsed),
            (3, "Drums", true)
        );
        assert!(encode(&groups, |_| false).is_empty());
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[9, 0, 0]), None);
    }
}
