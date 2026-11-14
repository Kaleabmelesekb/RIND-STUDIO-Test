//! Track groups, as in Ableton and Bitwig: a named, coloured set of channels the playlist can fold into one row.
//!
//! A group is a song-level list of channel ids ([`TrackGroup`]); a channel is in at most one. A group made here owns a
//! group bus: an `EffectTrack` channel that heads the group, that every member outputs to instead of the master, and
//! that has its own rack, fader, pan, mute and solo, so the members are processed together as in Ableton's group
//! track. A folded group shows its head row (the bus) alone and hides the rest, so a project of many tracks reads as
//! a few. Leaving a group sends a member back to the master; ungrouping deletes the bus, which sends its users there.
//!
//! The project file holds the groups in one `RGRP` chunk after the channels, written only while the song has a group
//! (a song without one is byte for byte what it was) and read back by [`decode`]; a reader that does not know the
//! chunk keeps it as an unknown one. A group from version 1 of the chunk has no bus: it only folds.

use super::{ChannelKind, SendState, Song, output_bus};

/// The most groups a song keeps, and the longest group name, in characters.
pub const MAX_GROUPS: usize = 256;
pub const MAX_NAME: usize = 48;

/// The `RGRP` chunk's version byte.
const VERSION: u8 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct TrackGroup {
    /// Unique in the song, never reused while the group lives.
    pub id: i32,
    pub name: String,
    /// The group's colour, a channel hue (see `theme::channel_color`).
    pub hue: f32,
    /// The playlist shows the group's head row alone.
    pub collapsed: bool,
    /// The channels in it, by [`super::Channel::id`]: the bus first, when there is one.
    pub members: Vec<i32>,
    /// The group bus, an `EffectTrack` channel's id that the members output to; 0 for a group without one.
    pub bus: i32,
}

impl TrackGroup {
    /// How many channels the group holds, not counting its bus.
    pub fn size(&self) -> usize {
        (self.members.iter())
            .filter(|&&m| self.bus == 0 || m != self.bus)
            .count()
    }
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

    /// The channel (index) heading `group`: its bus, or else its first member in the playlist.
    pub fn group_head(&self, group: &TrackGroup) -> Option<usize> {
        let members = self.group_channels(group);
        self.bus_index(group)
            .filter(|b| members.contains(b))
            .or_else(|| members.first().copied())
    }

    /// The group bus's channel index, if the group has one.
    pub fn bus_index(&self, group: &TrackGroup) -> Option<usize> {
        if group.bus == 0 {
            return None;
        }
        self.channels
            .iter()
            .position(|c| c.id == group.bus)
            .filter(|&c| c > 0)
    }

    /// The group's name: its bus channel's (which `Channel Settings` renames), else its own.
    pub fn group_name<'a>(&'a self, group: &'a TrackGroup) -> &'a str {
        match self.bus_index(group) {
            Some(b) => &self.channels[b].name,
            None => &group.name,
        }
    }

    /// The group's colour: its bus channel's, else its own.
    pub fn group_hue(&self, group: &TrackGroup) -> f32 {
        self.bus_index(group)
            .map_or(group.hue, |b| self.channels[b].hue)
    }

    /// Groups channel `channel` (an index) with the channel above it in the playlist: it joins that channel's group,
    /// or the two make a new one with a new group bus above them. Neither is the master, and the channel moves under
    /// the group's last member so the group stays together. The channel's output goes to the group bus. False, and no
    /// change, for the master, a first row, a group's own bus, or a channel already in that group.
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
        match self.group_of(above_id) {
            Some(g) if g.members.contains(&id) => return false,
            None if self.groups.len() >= MAX_GROUPS => return false,
            _ => {}
        }
        if self.group_of(id).is_some_and(|g| g.bus == id) {
            return false;
        }
        // Out of its old group first: that may dissolve it and delete its bus, so the indices are looked up again.
        self.leave_group(channel);
        let target = match self.group_of(above_id).map(|g| g.id) {
            Some(gid) => gid,
            None => {
                let gid = self.next_group_id();
                let name = self.free_group_name();
                let at = self.channels.iter().position(|c| c.id == above_id);
                let hue = at.map_or(0.0, |a| self.channels[a].hue);
                let bus = self
                    .insert_channel(self.channels.len(), ChannelKind::EffectTrack)
                    .map(|b| {
                        let c = &mut self.channels[b];
                        c.name = name.clone();
                        c.hue = hue;
                        c.id
                    })
                    .unwrap_or(0);
                let mut members = vec![above_id];
                if bus != 0 {
                    members.insert(0, bus);
                    self.route(above_id, bus);
                }
                self.groups.push(TrackGroup {
                    id: gid,
                    name,
                    hue,
                    collapsed: false,
                    members,
                    bus,
                });
                gid
            }
        };
        let bus = self.group(target).map_or(0, |g| g.bus);
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == target) {
            g.members.push(id);
        }
        if bus != 0 {
            self.route(id, bus);
        }
        self.gather_group(target);
        true
    }

    /// Takes channel `channel` (an index) out of its group, its output back to the master; a group left with one
    /// member or none is dissolved. On a group's own bus it ungroups the whole group. False, and no change, for a
    /// channel in no group.
    pub fn leave_group(&mut self, channel: usize) -> bool {
        let Some(id) = self.channels.get(channel).map(|c| c.id) else {
            return false;
        };
        let Some(at) = self.groups.iter().position(|g| g.members.contains(&id)) else {
            return false;
        };
        let (gid, bus) = (self.groups[at].id, self.groups[at].bus);
        if bus != 0 && bus == id {
            return self.ungroup(gid);
        }
        self.groups[at].members.retain(|&m| m != id);
        if bus != 0 {
            self.unroute(id, bus);
        }
        if self.groups[at].size() < 2 {
            self.ungroup(gid);
        }
        true
    }

    /// Dissolves the group `id`, its channels staying where they are. Its bus is deleted, and its members output to
    /// the master again.
    pub fn ungroup(&mut self, id: i32) -> bool {
        let Some(at) = self.groups.iter().position(|g| g.id == id) else {
            return false;
        };
        let group = self.groups.remove(at);
        if let Some(bus) = self.bus_index(&group) {
            self.delete_channel(bus);
        }
        true
    }

    /// Makes channel `id`'s main output the channel `to`: its send to the master (else its first send) goes there.
    fn route(&mut self, id: i32, to: i32) {
        let buses = self.send_buses();
        let Some(c) = self.channels.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let at = c
            .file
            .sends
            .iter()
            .position(|s| s.target_channel_id == 0)
            .unwrap_or(0);
        match c.file.sends.get_mut(at) {
            Some(send) => send.target_channel_id = to,
            None => c.file.sends.push(SendState {
                target_channel_id: to,
                ..SendState::default()
            }),
        }
        c.output_bus = output_bus(&c.file.sends, &buses);
    }

    /// Sends channel `id`'s output that goes to `from` back to the master (dropped where one to the master exists).
    fn unroute(&mut self, id: i32, from: i32) {
        let buses = self.send_buses();
        let Some(c) = self.channels.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let sends = &mut c.file.sends;
        if sends.iter().any(|s| s.target_channel_id == 0) {
            sends.retain(|s| s.target_channel_id != from);
        } else if let Some(send) = sends.iter_mut().find(|s| s.target_channel_id == from) {
            send.target_channel_id = 0;
        }
        c.output_bus = output_bus(sends, &buses);
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

    /// Moves the group's members next to each other in the playlist, at its first member's place: the bus on top,
    /// then the others in their playlist order.
    fn gather_group(&mut self, id: i32) {
        let Some(group) = self.group(id).cloned() else {
            return;
        };
        let bus = self.bus_index(&group);
        let all = self.group_channels(&group);
        let mut members: Vec<usize> = all.iter().copied().filter(|&c| Some(c) != bus).collect();
        let Some(&anchor) = members.first() else {
            return;
        };
        if let Some(b) = bus {
            members.insert(0, b);
        }
        let order = self.playlist();
        let mut gathered: Vec<usize> = Vec::with_capacity(order.len());
        for &c in &order {
            if c == anchor {
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
        // The bus, if it still exists.
        let bus = if exists(g.bus) { g.bus } else { 0 };
        out.extend(bus.to_le_bytes());
    }
    out
}

/// The groups an `RGRP` payload holds; `None` for a version this reader does not know or a payload cut short.
pub fn decode(bytes: &[u8]) -> Option<Vec<TrackGroup>> {
    let mut at: usize = 0;
    let mut take = |n: usize| {
        let part = bytes.get(at..at.checked_add(n)?)?;
        at += n;
        Some(part)
    };
    let version = *take(1)?.first()?;
    if !(1..=VERSION).contains(&version) {
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
        let bus = match version {
            1 => 0,
            _ => i32::from_le_bytes(take(4)?.try_into().ok()?),
        };
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
            bus,
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

    /// The index of the channel with id `id`.
    fn at(song: &Song, id: i32) -> usize {
        song.channels.iter().position(|c| c.id == id).unwrap()
    }

    /// Where channel `id` outputs: the id its first send names.
    fn target(song: &Song, id: i32) -> i32 {
        song.channels[at(song, id)].file.sends[0].target_channel_id
    }

    #[test]
    fn grouping_with_the_channel_above_makes_a_group_with_a_bus_and_the_next_joins_it() {
        let mut song = song();
        assert!(song.group_with_above(2));
        let g = song.groups[0].clone();
        assert_ne!(g.bus, 0);
        assert_eq!(g.members, [g.bus, 1, 2]);
        assert_eq!(g.name, "Group");
        let bus = &song.channels[at(&song, g.bus)];
        assert_eq!(
            (bus.kind, bus.name.as_str()),
            (ChannelKind::EffectTrack, "Group")
        );
        assert!(bus.bus_id >= 1);
        // The bus heads the group in the playlist, the members under it, the rest after.
        assert_eq!(ids(&song), [0, g.bus, 1, 2, 3, 4]);
        assert_eq!(song.group_head(&g), Some(at(&song, g.bus)));
        assert!(song.group_with_above(at(&song, 3)));
        assert_eq!(song.groups.len(), 1);
        assert_eq!(song.groups[0].size(), 3);
        assert_eq!(song.groups[0].bus, g.bus);
        // Already in it.
        assert!(!song.group_with_above(at(&song, 3)));
    }

    #[test]
    fn the_members_output_to_the_group_bus_and_the_bus_to_the_master() {
        let mut song = song();
        song.group_with_above(2);
        song.group_with_above(at(&song, 3));
        let bus = song.groups[0].bus;
        let bus_id = song.channels[at(&song, bus)].bus_id;
        for id in [1, 2, 3] {
            assert_eq!(target(&song, id), bus, "channel {id}");
            assert_eq!(song.channels[at(&song, id)].output_bus, bus_id);
        }
        // Not in the group: the master.
        assert_eq!(target(&song, 4), 0);
        assert_eq!(song.channels[at(&song, 4)].output_bus, -1);
        assert_eq!(target(&song, bus), 0);
    }

    #[test]
    fn the_master_and_the_first_row_do_not_group() {
        let mut song = song();
        assert!(!song.group_with_above(0));
        assert!(!song.group_with_above(1));
        assert!(song.groups.is_empty());
        assert_eq!(song.channels.len(), 5);
    }

    #[test]
    fn leaving_sends_a_channel_back_to_the_master_and_a_group_of_one_is_ungrouped() {
        let mut song = song();
        song.group_with_above(2);
        song.group_with_above(at(&song, 3));
        let bus = song.groups[0].bus;
        assert!(song.leave_group(at(&song, 3)));
        assert_eq!(target(&song, 3), 0);
        assert_eq!(song.groups[0].size(), 2);
        // The second of the last two leaves: the group and its bus are gone and the other is back on the master.
        assert!(song.leave_group(at(&song, 2)));
        assert!(song.groups.is_empty());
        assert!(song.channels.iter().all(|c| c.id != bus));
        assert_eq!(song.channels.len(), 5);
        for id in 1..=4 {
            assert_eq!(target(&song, id), 0, "channel {id}");
            assert_eq!(song.channels[at(&song, id)].output_bus, -1);
        }
        assert!(!song.leave_group(at(&song, 2)));
    }

    #[test]
    fn leave_on_the_bus_ungroups_everything() {
        let mut song = song();
        song.group_with_above(2);
        song.group_with_above(at(&song, 3));
        let bus = song.groups[0].bus;
        assert!(song.leave_group(at(&song, bus)));
        assert!(song.groups.is_empty());
        assert_eq!(song.channels.len(), 5);
        assert!((1..=3).all(|id| target(&song, id) == 0));
    }

    #[test]
    fn deleting_a_member_down_to_one_removes_the_group_and_its_bus() {
        let mut song = song();
        song.group_with_above(2);
        let bus = song.groups[0].bus;
        song.delete_channel(at(&song, 2));
        assert!(song.groups.is_empty());
        assert!(song.channels.iter().all(|c| c.id != bus));
        assert_eq!(target(&song, 1), 0);
        assert_eq!(ids(&song), [0, 1, 3, 4]);
    }

    #[test]
    fn deleting_the_bus_keeps_the_members_as_a_plain_group_on_the_master() {
        let mut song = song();
        song.group_with_above(2);
        song.group_with_above(at(&song, 3));
        let bus = song.groups[0].bus;
        song.delete_channel(at(&song, bus));
        assert_eq!(song.groups.len(), 1);
        assert_eq!(song.groups[0].bus, 0);
        assert_eq!(song.groups[0].members, [1, 2, 3]);
        assert!((1..=3).all(|id| target(&song, id) == 0));
    }

    #[test]
    fn folding_hides_nothing_in_the_model_but_flags_the_group() {
        let mut song = song();
        song.group_with_above(2);
        let id = song.groups[0].id;
        assert!(song.set_group_collapsed(id, true));
        assert!(!song.set_group_collapsed(id, true));
        let g = song.group(id).unwrap().clone();
        assert_eq!(song.group_head(&g), Some(at(&song, g.bus)));
        assert_eq!(song.group_channels(&g).len(), 3);
        assert!(song.ungroup(id));
        assert!(!song.ungroup(id));
        assert_eq!(song.channels.len(), 5);
    }

    #[test]
    fn the_group_takes_its_buss_name_and_colour() {
        let mut song = song();
        song.group_with_above(2);
        let bus = at(&song, song.groups[0].bus);
        song.channels[bus].name = "Drums".into();
        song.channels[bus].hue = 0.5;
        let g = song.groups[0].clone();
        assert_eq!(song.group_name(&g), "Drums");
        assert_eq!(song.group_hue(&g), 0.5);
    }

    #[test]
    fn the_chunk_round_trips_and_leaves_out_what_is_gone() {
        let groups = vec![TrackGroup {
            id: 3,
            name: "Drums".into(),
            hue: 0.25,
            collapsed: true,
            members: vec![7, 1, 2, 9],
            bus: 7,
        }];
        let bytes = encode(&groups, |id| id != 9);
        let back = decode(&bytes).unwrap();
        assert_eq!(back[0].members, [7, 1, 2]);
        assert_eq!(back[0].bus, 7);
        assert_eq!(
            (back[0].id, back[0].name.as_str(), back[0].collapsed),
            (3, "Drums", true)
        );
        // A bus that is gone is left out.
        assert_eq!(decode(&encode(&groups, |id| id != 7)).unwrap()[0].bus, 0);
        assert!(encode(&groups, |_| false).is_empty());
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[9, 0, 0]), None);
    }

    #[test]
    fn a_version_1_chunk_reads_as_groups_without_a_bus() {
        let mut bytes = vec![1u8];
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(5i32.to_le_bytes());
        bytes.extend(0.5f32.to_le_bytes());
        bytes.push(1);
        bytes.extend(1u16.to_le_bytes());
        bytes.push(b'G');
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(1i32.to_le_bytes());
        bytes.extend(2i32.to_le_bytes());
        let groups = decode(&bytes).unwrap();
        assert_eq!(groups[0].members, [1, 2]);
        assert_eq!((groups[0].bus, groups[0].collapsed), (0, true));
    }
}
