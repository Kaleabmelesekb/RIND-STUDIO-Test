// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(semicolon_in_expressions_from_non_local_macros)]
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
pub type Effect = ml_mix_core::shaper::Waveshaper;
#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);
