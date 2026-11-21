// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Lemon Studio's small effects as VST3 / CLAP plugins (nih-plug): Lemon Soft Clipper, Lemon Fast Distortion, Lemon
//! Balance, Lemon Stereo Enhancer, Lemon Transient Processor, Lemon Chorus and Lemon Stereo Shaper. Each is a handful
//! of settings over a sound of
//! `os_engine::effects`, so they share
//! everything but their description: the plugin, its parameters and saved state ([`params`]), the Pith editor
//! ([`editor`], [`view`]). A description ([`Spec`]) gives the settings, their value texts, the editor's cards and
//! controls, and the sound. The plugin modules (`plugins/lemon-soft-clipper` and the others) export one each.
//!
// spec: plugins/lemon-small-effects.md (§1 how they are built, §5 parameters and state), the plugin pages
// (plugins/lemon-soft-clipper.md, lemon-fast-distortion.md, lemon-balance.md, lemon-stereo-enhancer.md,
// lemon-transient-processor.md, lemon-chorus.md, lemon-stereo-shaper.md), plugins/plugin-formats.md
#![allow(semicolon_in_expressions_from_non_local_macros)]

pub mod tape;
pub mod editor;
pub mod state_undo;
pub mod params;
pub mod view;
pub use os_plugkit::visibility::STOCK_DSP_CONTROLS;

use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use nih_plug::prelude::*;
use os_ui::{Canvas, Color, Rect};

use params::FxParams;

/// Capacity of shared parameter/editor snapshots, including Lemon Wave.
pub const MAX: usize = 384;

/// One setting: the original's saved state holds it as int `k` (its place in [`Spec::settings`], unless
/// [`Spec::fields`] maps it).
pub struct Setting {
    /// The parameter's id (host automation and saved sessions).
    pub id: &'static str,
    /// The name hosts and the hint show.
    pub name: &'static str,
    /// The stored range, also the knob's.
    pub range: (i32, i32),
    /// A new instance's value.
    pub start: i32,
    /// Where a drag lands when it comes near and a wheel step stops (the centre of a bipolar knob, unity gain).
    pub magnet: Option<i32>,
}

/// A control of the editor.
pub enum Control {
    /// A Pith dial of radius `r` centred at (`cx`, `cy`), its caption under it (the value text, in lemon, when the
    /// caption is empty).
    Knob { id: &'static str, cx: f32, cy: f32, r: f32, caption: &'static str, bipolar: bool },
    /// A radio list: one row per value from the range's minimum, 16 px apart from `top`, a toggle at `x` and the name
    /// beside it.
    Choice { id: &'static str, x: f32, top: f32, names: &'static [&'static str] },
    /// A vertical fader over `top..bot` at `x`, its fill in `col` from the bottom (from the middle when `bipolar`),
    /// `label` over it and ticks at `marks` (normalised).
    Fader { id: &'static str, x: f32, top: f32, bot: f32, label: &'static str, col: Color, bipolar: bool, marks: &'static [f32] },
    /// A button centred at (`cx`, `cy`) that shows the value's envelope shape; a click steps to the next value.
    Shape { id: &'static str, cx: f32, cy: f32 },
    /// A tile (Lemon Chorus's grid): its own card over `x0..x1`, `y0..y1`, a small dial at the left, the caption in a
    /// field at the top right and the value text under it.
    Tile { id: &'static str, x0: f32, y0: f32, x1: f32, y1: f32, caption: &'static str },
}

impl Control {
    pub fn id(&self) -> &'static str {
        match self {
            Control::Knob { id, .. }
            | Control::Choice { id, .. }
            | Control::Fader { id, .. }
            | Control::Shape { id, .. }
            | Control::Tile { id, .. } => id,
        }
    }
}

/// Where the wordmark goes: the lemon mark and LEMON beside the name on one line centred at `y`, or (`stacked`) LEMON
/// over the name, its top at `y`. `anchor` is the share of its width left of `x` (1 = right edge on `x`).
pub struct Mark {
    pub x: f32,
    pub y: f32,
    pub anchor: f32,
    pub stacked: bool,
}

/// An outlined card under a ruled heading (no heading when it is empty).
pub struct Card {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub heading: &'static str,
}

/// A well that pictures the settings (a transfer curve, the output balance).
pub struct Display {
    pub r: Rect,
    pub draw: fn(&mut Canvas, Rect, &[i32]),
}

/// The sound of an effect, made for a sample rate from the settings in the original's order.
pub trait Sound: Send {
    /// New settings: they act from this block on (gains glide where the original's do).
    fn set(&mut self, v: &[i32]);
    fn set_sample_rate(&mut self, rate: u32);
    /// Forgets what the sound holds (a delay line), as at the start of a song.
    fn reset(&mut self);
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);
}

/// Everything that makes one of the effects.
pub struct Spec {
    /// "ML Soft Clipper": the plugin's name.
    pub name: &'static str,
    /// The wordmark's word: "SOFT CLIPPER".
    pub word: &'static str,
    /// The About menu's line: "ML Studio's soft clipper".
    pub about: &'static str,
    /// The settings in the original's saved order.
    pub settings: &'static [Setting],
    /// Number of ML-specific settings at the end of the list.
    pub studio_settings: usize,
    /// The version the original's saved state starts with (an int before the settings), if it has one. A stored
    /// state with a higher version is not read (spec §5), unless `any_version`.
    pub version: Option<i32>,
    /// Every version of the saved state is read the same way (Fruity Stereo Shaper's).
    pub any_version: bool,
    /// The int (after the version) each setting is, when that is not its place: an int left out is kept as stored
    /// (Lemon Stereo Shaper leaves out the difference output). Empty: setting k is int k.
    pub fields: &'static [usize],
    /// The bytes a new instance's saved state has after the settings (kept as stored otherwise).
    pub tail: &'static [u8],
    /// Editor size in logical pixels.
    pub size: (u32, u32),
    pub cards: &'static [Card],
    pub controls: &'static [Control],
    pub display: Option<Display>,
    pub mark: Mark,
    /// The hint well (`None`: the hint goes in the display's top-left corner); on the wordmark's line it stops 16 px
    /// short of it.
    pub hint: Option<Rect>,
    /// Captions that belong to no control: (centre x, top, text).
    pub labels: &'static [(f32, f32, &'static str)],
    /// A setting's value text.
    pub text: fn(usize, i32) -> String,
    /// A typed value for a setting.
    pub parse: fn(usize, &str) -> Option<i32>,
    pub sound: fn(u32, &[i32]) -> Box<dyn Sound>,
    /// The sound rings on after the input stops (a delay line).
    pub rings: bool,
}

impl Spec {
    /// Setting index of an id.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.settings.iter().position(|s| s.id == id)
    }

    pub fn legacy_settings(&self) -> usize {
        self.settings.len() - self.studio_settings
    }

    /// The settings a new instance starts on.
    pub fn start(&self) -> Vec<i32> {
        self.settings.iter().map(|s| s.start).collect()
    }
}

/// What makes a plugin type of one of the effects: its description and its ids.
pub trait Kind: 'static + Send + Sync {
    const SPEC: &'static Spec;
    const CLAP_ID: &'static str;
    const CLAP_DESCRIPTION: &'static str;
    const CLAP_FEATURES: &'static [ClapFeature];
    const VST3_CLASS_ID: [u8; 16];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory];
}

/// What the audio thread and the editor share.
pub struct Shared {
    /// Optional instrument-specific controls; ordinary effects leave this unset.
    pub extension: Option<Arc<dyn editor::Extension>>,
    pub rate: AtomicU32,
    /// Set while an editor is open.
    pub editor_open: AtomicBool,
}

impl Default for Shared {
    fn default() -> Self {
        Shared { extension: None, rate: AtomicU32::new(44100), editor_open: AtomicBool::new(false) }
    }
}

/// One of the effects as a plugin.
pub struct SmallFx<K: Kind> {
    params: Arc<FxParams>,
    shared: Arc<Shared>,
    /// Made in `initialize` (for the sample rate).
    dsp: Option<Box<dyn Sound>>,
    raw: Vec<u8>,
    raw_seen: u64,
    kind: PhantomData<K>,
}

impl<K: Kind> Default for SmallFx<K> {
    fn default() -> Self {
        SmallFx {
            params: Arc::new(FxParams::new(K::SPEC)),
            shared: Arc::new(Shared::default()),
            dsp: None,
            raw: params::default_state(K::SPEC),
            raw_seen: u64::MAX,
            kind: PhantomData,
        }
    }
}

impl<K: Kind> SmallFx<K> {
    /// The saved state's copy (never blocks: keeps the last one while the editor writes).
    fn read_raw(&mut self) {
        let v = self.params.raw_version.load(Ordering::Acquire);
        if v == self.raw_seen {
            return;
        }
        if let Ok(b) = self.params.raw.try_read() {
            self.raw.clone_from(&b);
            self.raw_seen = v;
        }
    }

    /// The settings the sound plays (the first `n` values; no allocation).
    fn settings(&mut self) -> [i32; MAX] {
        self.read_raw();
        self.params.settings(&self.raw)
    }
}

impl<K: Kind> Plugin for SmallFx<K> {
    const NAME: &'static str = K::SPEC.name;
    const VENDOR: &'static str = "ML Studio";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] =
        &[AudioIOLayout { main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() }];
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(editor::FxEditor::new(K::SPEC, self.params.clone(), self.shared.clone())))
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, config: &BufferConfig, _ctx: &mut impl InitContext<Self>) -> bool {
        let rate = config.sample_rate.round() as u32;
        self.shared.rate.store(rate, Ordering::Relaxed);
        self.raw_seen = u64::MAX;
        let s = self.settings();
        let v = &s[..K::SPEC.settings.len()];
        match &mut self.dsp {
            Some(d) => {
                d.set_sample_rate(rate);
                d.set(v);
            }
            None => self.dsp = Some((K::SPEC.sound)(rate, v)),
        }
        true
    }

    fn reset(&mut self) {
        if let Some(d) = &mut self.dsp {
            d.reset();
        }
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, _ctx: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let s = self.settings();
        let Some(dsp) = &mut self.dsp else { return ProcessStatus::Normal };
        dsp.set(&s[..K::SPEC.settings.len()]);
        let out = buffer.as_slice();
        if out.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (left, right) = out.split_at_mut(1);
        dsp.process(left[0], right[0]);
        if K::SPEC.rings { ProcessStatus::KeepAlive } else { ProcessStatus::Normal }
    }
}

impl<K: Kind> ClapPlugin for SmallFx<K> {
    const CLAP_ID: &'static str = K::CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(K::CLAP_DESCRIPTION);
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = K::CLAP_FEATURES;
}

impl<K: Kind> Vst3Plugin for SmallFx<K> {
    const VST3_CLASS_ID: [u8; 16] = K::VST3_CLASS_ID;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = K::VST3_SUBCATEGORIES;
}
