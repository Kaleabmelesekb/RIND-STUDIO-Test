// SPDX-License-Identifier: GPL-3.0-or-later
//! Rind OB's editor: the synth on four pages (the sound, the modulation, the voice, the patches), drawn in the ML
//! Studio look. Knobs and switches are the host's parameters (the editor drags them); the tabs and the patch list are
//! this extension's.
use std::sync::{Arc, Mutex};

use ml_small_fx::editor::Extension;
use ml_small_fx::params::{self, FxParams};
use ml_small_fx::view::{Hit, Model, Target, ViewState};
use nih_plug::prelude::GuiContext;
use os_plugkit::vals::Vals;
use os_theme::design::lemon as brand;
use os_ui::kit::lemon::{self, plug};
use os_ui::kit::State;
use os_ui::{Canvas, Color, Font, Rect};

use crate::presets;
use crate::settings::*;
use crate::{LEGATO, PRIORITY};

const LABEL: Font = Font::new(10).bold();
const CAP: Font = Font::new(10);
const ROW: Font = Font::new(13);
const TABS: [&str; 4] = ["SOUND", "MOD", "VOICE", "PATCHES"];
const PATCHES_PAGE: usize = 3;
const PER_PAGE: usize = 32;
/// The place a patch has in the saved state when no patch is loaded.
const NONE: u16 = 0xFFFF;

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Page(usize),
    /// The category list: 0 is all, then the categories.
    Category(usize),
    Patch(usize),
    Step(i32),
    List(i32),
    Init,
}

struct St {
    page: usize,
    category: usize,
    list: usize,
    acts: Vec<(Rect, Act)>,
}

pub struct Panel {
    params: Arc<FxParams>,
    st: Mutex<St>,
}

/// The patch the saved state says is loaded.
fn info(params: &FxParams) -> Option<usize> {
    let raw = params.raw_copy();
    let at = params::state_len(&crate::SPEC);
    let t = raw.get(at..at + 2)?;
    let i = u16::from_le_bytes([t[0], t[1]]);
    (i != NONE && usize::from(i) < presets::bank().presets.len()).then_some(usize::from(i))
}

/// Whether a setting is not what the loaded patch has.
fn edited(m: &dyn Model, index: usize) -> bool {
    presets::bank().presets.get(index).is_some_and(|p| {
        p.values.iter().enumerate().take(COUNT).any(|(k, &v)| m.plain(IDS[k]).round() as i32 != i32::from(v))
    })
}

fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}

impl Panel {
    pub fn new(params: Arc<FxParams>) -> Arc<dyn Extension> {
        let category = info(&params).map_or(0, |i| presets::bank().presets[i].category + 1);
        Arc::new(Panel { params, st: Mutex::new(St { page: 0, category, list: 0, acts: Vec::new() }) })
    }

    fn set_info(&self, index: Option<usize>) {
        let mut raw = self.params.raw_copy();
        let at = params::state_len(&crate::SPEC);
        raw.resize(at + 2, 0);
        let i = index.map_or(NONE, |i| i.min(0xFFFE) as u16);
        raw[at..at + 2].copy_from_slice(&i.to_le_bytes());
        self.params.set_raw(raw);
    }

    /// Every setting to the patch's value: the sound hears it at once and the host is told.
    fn load(&self, index: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let Some(p) = presets::bank().presets.get(index) else { return };
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        for (k, &v) in p.values.iter().enumerate().take(COUNT) {
            vals.edit_plain(IDS[k], f32::from(v));
        }
        self.set_info(Some(index));
    }

    fn init(&self, ctx: Option<&Arc<dyn GuiContext>>) {
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        for (k, s) in SETTINGS.iter().enumerate() {
            vals.edit_plain(IDS[k], s.start as f32);
        }
        self.set_info(None);
    }

    /// The patches the category list shows, as places in the bank.
    fn shown(category: usize) -> Vec<usize> {
        let b = presets::bank();
        (0..b.presets.len()).filter(|&i| category == 0 || b.presets[i].category + 1 == category).collect()
    }
}

impl Extension for Panel {
    fn paint(&self, _: &mut Canvas, _: &dyn Model) {}

    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        let Ok(mut st) = self.st.lock() else { return false };
        let Some(&(_, act)) = st.acts.iter().rev().find(|(r, _)| r.contains(x, y)) else { return false };
        match act {
            Act::Page(p) => st.page = p,
            Act::Category(c) => {
                st.category = c;
                st.list = 0;
            }
            Act::Patch(i) => self.load(i, ctx),
            Act::Step(d) => {
                let n = presets::bank().presets.len() as i32;
                if n > 0 {
                    let next = (info(&self.params).map_or(if d > 0 { -1 } else { 0 }, |i| i as i32) + d).rem_euclid(n) as usize;
                    self.load(next, ctx);
                    st.category = presets::bank().presets[next].category + 1;
                }
            }
            Act::List(d) => {
                let pages = Self::shown(st.category).len().div_ceil(PER_PAGE).max(1) as i32;
                st.list = (st.list as i32 + d).rem_euclid(pages) as usize;
            }
            Act::Init => self.init(ctx),
        }
        true
    }

    fn render(&self, c: &mut Canvas, m: &dyn Model, state: &ViewState, _hint: &str) -> Option<Vec<Hit>> {
        let (page, category, list) = {
            let st = self.st.lock().ok()?;
            (st.page, st.category, st.list)
        };
        let loaded = info(&self.params);
        let changed = loaded.is_some_and(|i| edited(m, i));
        let mut ui = Ui { c, m, st: state, hits: Vec::new(), acts: Vec::new() };
        ui.c.begin_frame(None);
        ui.c.clear(plug::BG);
        for (i, name) in TABS.iter().enumerate() {
            ui.button(Rect::new(8.0 + 106.0 * i as f32, 8.0, 100.0, 28.0), name, Act::Page(i), page == i);
        }
        ui.lcd(loaded, changed);
        ui.master();
        match page {
            0 => ui.sound(),
            1 => ui.modulation(),
            2 => ui.voice(),
            _ => ui.patches(category, list, loaded),
        }
        let (hits, acts) = (ui.hits, ui.acts);
        if let Ok(mut st) = self.st.lock() {
            st.acts = acts;
        }
        Some(hits)
    }
}

struct Ui<'a> {
    c: &'a mut Canvas,
    m: &'a dyn Model,
    st: &'a ViewState,
    hits: Vec<Hit>,
    acts: Vec<(Rect, Act)>,
}

impl Ui<'_> {
    fn hover(&self, r: Rect) -> bool {
        self.st.pressed.is_none() && self.st.mouse.is_some_and(|(x, y)| r.contains(x, y))
    }

    fn state(&self, r: Rect, t: &Target) -> State {
        let pressed = self.st.pressed.as_ref() == Some(t);
        State { hover: !pressed && self.hover(r), pressed, ..State::NORMAL }
    }

    fn button(&mut self, r: Rect, label: &str, act: Act, on: bool) {
        let st = State { hover: self.hover(r), on, ..State::NORMAL };
        let ink = lemon::plug_button(self.c, r, 6.0, st);
        self.c.text_center(Font::new(12).bold(), r, ink, label);
        self.acts.push((r, act));
    }

    fn card(&mut self, r: Rect, heading: &str) {
        lemon::plug_panel(self.c, r);
        let w = lemon::spaced_width(self.c, LABEL, heading, 2.0);
        lemon::spaced(self.c, LABEL, r.x + 12.0, r.y + 7.0, plug::DIM, heading, 2.0);
        self.c.fill_rect(Rect::span(r.x + 12.0 + w + 8.0, r.right() - 10.0, r.y + 13.0, r.y + 14.0), Color::rgb(0x3c3c3c));
    }

    /// A dial for setting `k` with its caption over it and its value under it.
    fn knob(&mut self, k: usize, cx: f32, cy: f32, r: f32, caption: &str) {
        let id = IDS[k];
        let target = Target::Knob(id.to_string());
        let hr = Rect::new(cx - r - 4.0, cy - r - 4.0, 2.0 * r + 8.0, 2.0 * r + 8.0);
        let st = self.state(hr, &target);
        lemon::dial(self.c, cx, cy, r, 3.2, self.m.norm(id), SETTINGS[k].magnet.is_some(), brand::MAIN, st);
        self.c.text_center(CAP, Rect::new(cx - 44.0, cy - r - 17.0, 88.0, 13.0), plug::DIM, caption);
        let text = self.m.text(id);
        self.c.text_center(LABEL, Rect::new(cx - 44.0, cy + r + 4.0, 88.0, 13.0), brand::MAIN, &text);
        self.hits.push(Hit { r: hr, t: target });
    }

    /// An on/off setting as a switch with its name beside it.
    fn switch(&mut self, k: usize, x: f32, cy: f32, label: &str) {
        let id = IDS[k];
        let on = self.m.plain(id).round() as i32 >= 500;
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), if on { 0 } else { 1000 });
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    /// One position of a selector setting: `value` is what it stores, `on` whether the setting is at it now.
    fn option(&mut self, k: usize, value: i32, on: bool, x: f32, cy: f32, label: &str) {
        let id = IDS[k];
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), value);
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    fn value(&self, k: usize) -> i32 {
        self.m.plain(IDS[k]).round() as i32
    }

    fn lcd(&mut self, loaded: Option<usize>, changed: bool) {
        let r = Rect::new(8.0, 44.0, 300.0, 60.0);
        lemon::plug_well(self.c, r, 6.0);
        let glow = Color::rgb(0xb7e36a);
        let dim = Color::rgb(0x7f9a54);
        let bank = presets::bank();
        let (head, sub) = match loaded.and_then(|i| bank.presets.get(i)) {
            Some(p) => {
                let author = bank.authors.get(p.author).map_or("", |s| s.as_str());
                let cat = bank.categories.get(p.category).map_or("", |s| s.as_str());
                let tag = if changed { "  EDITED" } else { "" };
                (trim(&p.name, 22), trim(&format!("{}  {}{}", cat.to_uppercase(), author.to_uppercase(), tag), 40))
            }
            None => ("INIT".to_string(), "EDITED PATCH".to_string()),
        };
        self.c.text(Font::new(20).bold(), r.x + 14.0, r.y + 8.0, glow, &head);
        self.c.text(CAP, r.x + 16.0, r.y + 38.0, dim, &sub);
        self.button(Rect::new(314.0, 44.0, 32.0, 60.0), "<", Act::Step(-1), false);
        self.button(Rect::new(350.0, 44.0, 32.0, 60.0), ">", Act::Step(1), false);
    }

    /// The top bar on every page: the volume, the tune, the transpose, high quality and unison.
    fn master(&mut self) {
        lemon::plug_panel(self.c, Rect::new(388.0, 44.0, 364.0, 60.0));
        for (i, (k, cap)) in [(VOLUME, "VOLUME"), (TUNE, "TUNE"), (TRANSPOSE, "TRANSPOSE")].iter().enumerate() {
            self.knob(*k, 424.0 + 62.0 * i as f32, 74.0, 11.0, cap);
        }
        self.switch(HQ_MODE, 600.0, 74.0, "HQ");
        self.switch(UNISON, 660.0, 74.0, "UNISON");
    }

    fn sound(&mut self) {
        // Oscillators.
        self.card(Rect::new(8.0, 112.0, 364.0, 170.0), "OSCILLATORS");
        let knobs = [
            (OSC1_PITCH, "OSC 1"), (OSC2_PITCH, "OSC 2"), (OSC2_DETUNE, "DETUNE"), (OSC_PW, "PW"), (OSC2_PW_OFFSET, "PW OFS 2"),
            (OSC_CROSSMOD, "CROSSMOD"),
        ];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(*k, 44.0 + 58.0 * i as f32, 176.0, 16.0, cap);
        }
        self.switch(OSC1_SAW_WAVE, 24.0, 238.0, "1 SAW");
        self.switch(OSC1_PULSE_WAVE, 100.0, 238.0, "1 PULSE");
        self.switch(OSC2_SAW_WAVE, 188.0, 238.0, "2 SAW");
        self.switch(OSC2_PULSE_WAVE, 264.0, 238.0, "2 PULSE");
        self.switch(OSC_SYNC, 24.0, 266.0, "SYNC");
        self.switch(OSC2_KEYTRACK, 100.0, 266.0, "2 KEYTRACK");
        // Mixer.
        self.card(Rect::new(388.0, 112.0, 364.0, 170.0), "MIXER");
        let knobs = [(OSC1_MIX, "OSC 1"), (OSC2_MIX, "OSC 2"), (RING_MOD_MIX, "RING"), (NOISE_MIX, "NOISE"), (OSC_BRIGHTNESS, "BRIGHT")];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(*k, 426.0 + 68.0 * i as f32, 176.0, 16.0, cap);
        }
        let colour = noise_at(self.value(NOISE_COLOR));
        for (i, (v, name)) in [(100, "WHITE"), (500, "PINK"), (900, "RED")].iter().enumerate() {
            self.option(NOISE_COLOR, *v, colour == i, 404.0 + 76.0 * i as f32, 238.0, name);
        }
        // Filter.
        self.card(Rect::new(8.0, 290.0, 364.0, 262.0), "FILTER");
        let knobs = [
            (FILTER_CUTOFF, "CUTOFF"), (FILTER_RESONANCE, "RESONANCE"), (FILTER_ENV_AMOUNT, "ENV AMT"), (FILTER_KEY_FOLLOW, "KEYTRACK"),
            (FILTER_MODE, "LP-BP-HP"),
        ];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(*k, 46.0 + 68.0 * i as f32, 366.0, 18.0, cap);
        }
        self.knob(FILTER_XPANDER_MODE, 46.0, 470.0, 18.0, "XPANDER MODE");
        self.switch(FILTER4_POLE_MODE, 140.0, 440.0, "4-POLE");
        self.switch(FILTER4_POLE_XPANDER, 230.0, 440.0, "XPANDER");
        self.switch(FILTER2_POLE_PUSH, 140.0, 476.0, "2P PUSH");
        self.switch(FILTER2_POLE_BP_BLEND, 230.0, 476.0, "2P BP BLEND");
        // Envelopes.
        self.card(Rect::new(388.0, 290.0, 364.0, 125.0), "FILTER ENVELOPE");
        self.switch(FILTER_ENV_INVERT, 664.0, 296.0, "INVERT");
        let env = [
            (FILTER_ENV_ATTACK, "ATTACK"), (FILTER_ENV_DECAY, "DECAY"), (FILTER_ENV_SUSTAIN, "SUSTAIN"), (FILTER_ENV_RELEASE, "RELEASE"),
            (FILTER_ENV_ATTACK_CURVE, "CURVE"), (VEL_TO_FILTER_ENV, "VELOCITY"),
        ];
        for (i, (k, cap)) in env.iter().enumerate() {
            self.knob(*k, 424.0 + 56.0 * i as f32, 354.0, 15.0, cap);
        }
        self.card(Rect::new(388.0, 423.0, 364.0, 129.0), "AMP ENVELOPE");
        let env = [
            (AMP_ENV_ATTACK, "ATTACK"), (AMP_ENV_DECAY, "DECAY"), (AMP_ENV_SUSTAIN, "SUSTAIN"), (AMP_ENV_RELEASE, "RELEASE"),
            (AMP_ENV_ATTACK_CURVE, "CURVE"), (VEL_TO_AMP_ENV, "VELOCITY"),
        ];
        for (i, (k, cap)) in env.iter().enumerate() {
            self.knob(*k, 424.0 + 56.0 * i as f32, 491.0, 15.0, cap);
        }
    }

    fn lfo(&mut self, x: f32, n: usize) {
        let o = if n == 1 { 0 } else { LFO2_TEMPO_SYNC - LFO1_TEMPO_SYNC };
        self.card(Rect::new(x, 112.0, 364.0, 300.0), &format!("LFO {n}"));
        let knobs = [
            (LFO1_RATE, "RATE"), (LFO1_MOD_AMOUNT1, "AMT 1"), (LFO1_MOD_AMOUNT2, "AMT 2"), (LFO1_WAVE1, "SIN/TRI"),
            (LFO1_WAVE2, "PLS/SAW"), (LFO1_WAVE3, "S&H/GL"), (LFO1_PW, "PW"),
        ];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(k + o, x + 30.0 + 50.0 * i as f32, 176.0, 15.0, cap);
        }
        self.switch(LFO1_TEMPO_SYNC + o, x + 20.0, 232.0, "TEMPO SYNC");
        let dests = [
            (LFO1_TO_OSC1_PITCH, "OSC 1 PITCH"), (LFO1_TO_OSC2_PITCH, "OSC 2 PITCH"), (LFO1_TO_FILTER_CUTOFF, "CUTOFF"),
            (LFO1_TO_OSC1_PW, "OSC 1 PW"), (LFO1_TO_OSC2_PW, "OSC 2 PW"), (LFO1_TO_VOLUME, "VOLUME"),
        ];
        for (i, (k, name)) in dests.iter().enumerate() {
            let cy = 266.0 + 28.0 * i as f32;
            self.c.text(LABEL, x + 20.0, cy - 6.0, plug::DIM, name);
            let at = crate::tri(self.value(k + o));
            for (j, (v, label)) in [(0, "OFF"), (500, "ON"), (1000, "INV")].iter().enumerate() {
                self.option(k + o, *v, at == j, x + 170.0 + 62.0 * j as f32, cy, label);
            }
        }
    }

    fn modulation(&mut self) {
        self.lfo(8.0, 1);
        self.lfo(388.0, 2);
        self.card(Rect::new(8.0, 424.0, 744.0, 128.0), "ENVELOPE MOD AND VIBRATO");
        self.knob(ENV_TO_PITCH_AMOUNT, 50.0, 496.0, 16.0, "ENV > PITCH");
        self.switch(ENV_TO_PITCH_BOTH_OSCS, 100.0, 478.0, "BOTH OSCS");
        self.switch(ENV_TO_PITCH_INVERT, 100.0, 510.0, "INVERT");
        self.knob(ENV_TO_PW_AMOUNT, 250.0, 496.0, 16.0, "ENV > PW");
        self.switch(ENV_TO_PW_BOTH_OSCS, 300.0, 478.0, "BOTH OSCS");
        self.switch(ENV_TO_PW_INVERT, 300.0, 510.0, "INVERT");
        self.knob(VIBRATO_RATE, 470.0, 496.0, 16.0, "VIBRATO RATE");
        let square = self.value(VIBRATO_WAVE) >= 500;
        self.option(VIBRATO_WAVE, 0, !square, 530.0, 478.0, "SINE");
        self.option(VIBRATO_WAVE, 1000, square, 530.0, 510.0, "SQUARE");
    }

    fn voice(&mut self) {
        self.card(Rect::new(8.0, 112.0, 364.0, 256.0), "GLOBAL");
        let knobs = [(POLYPHONY, "POLYPHONY"), (PORTAMENTO, "GLIDE"), (UNISON_VOICES, "UNISON VOICES"), (UNISON_DETUNE, "UNISON DETUNE")];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(*k, 46.0 + 84.0 * i as f32, 178.0, 18.0, cap);
        }
        self.switch(UNISON, 24.0, 252.0, "UNISON");
        self.switch(VOICE_REASSIGN, 120.0, 252.0, "VOICE REASSIGN");
        self.c.text(LABEL, 24.0, 284.0, plug::DIM, "LEGATO");
        let at = crate::legato(self.value(ENV_LEGATO_MODE));
        for (i, name) in LEGATO.iter().enumerate() {
            let x = [90.0, 150.0, 214.0, 264.0][i];
            self.option(ENV_LEGATO_MODE, [100, 400, 700, 1000][i], at == i, x, 290.0, &name.to_uppercase());
        }
        self.c.text(LABEL, 24.0, 324.0, plug::DIM, "PRIORITY");
        let at = crate::priority(self.value(NOTE_PRIORITY));
        for (i, name) in PRIORITY.iter().enumerate() {
            self.option(NOTE_PRIORITY, [100, 500, 1000][i], at == i, 90.0 + 70.0 * i as f32, 330.0, &name.to_uppercase());
        }
        self.card(Rect::new(388.0, 112.0, 364.0, 120.0), "PITCH BEND");
        self.knob(PITCH_BEND_UP, 440.0, 178.0, 16.0, "UP");
        self.knob(PITCH_BEND_DOWN, 510.0, 178.0, 16.0, "DOWN");
        self.switch(BEND_OSC2_ONLY, 568.0, 178.0, "OSC 2 ONLY");
        self.card(Rect::new(388.0, 240.0, 364.0, 128.0), "ANALOGUE SLOP");
        let knobs = [(PORTAMENTO_SLOP, "GLIDE"), (FILTER_SLOP, "FILTER"), (ENVELOPE_SLOP, "ENVELOPE"), (LEVEL_SLOP, "LEVEL")];
        for (i, (k, cap)) in knobs.iter().enumerate() {
            self.knob(*k, 428.0 + 84.0 * i as f32, 308.0, 16.0, cap);
        }
        self.card(Rect::new(8.0, 376.0, 744.0, 176.0), "VOICE PANNING");
        for i in 0..8 {
            self.knob(PAN_VOICE1 + i, 62.0 + 90.0 * i as f32, 462.0, 18.0, &format!("VOICE {}", i + 1));
        }
    }

    fn patches(&mut self, category: usize, list: usize, loaded: Option<usize>) {
        let bank = presets::bank();
        let names = std::iter::once("ALL".to_string()).chain(bank.categories.iter().map(|c| c.to_uppercase())).collect::<Vec<_>>();
        for (i, name) in names.iter().enumerate() {
            let r = Rect::new(8.0, 112.0 + 22.5 * i as f32, 170.0, 21.0);
            let st = State { hover: self.hover(r), on: category == i, ..State::NORMAL };
            lemon::field(self.c, r, st);
            self.c.text(LABEL, r.x + 10.0, r.y + 5.0, if category == i { brand::MAIN } else { brand::TEXT }, name);
            self.acts.push((r, Act::Category(i)));
        }
        let shown = Panel::shown(category);
        let pages = shown.len().div_ceil(PER_PAGE).max(1);
        let list = list.min(pages - 1);
        for (n, &i) in shown.iter().skip(list * PER_PAGE).take(PER_PAGE).enumerate() {
            let (col, row) = (n / 16, n % 16);
            let r = Rect::new(186.0 + 285.0 * col as f32, 112.0 + 24.5 * row as f32, 277.0, 23.0);
            let here = loaded == Some(i);
            let st = State { hover: self.hover(r), on: here, ..State::NORMAL };
            lemon::field(self.c, r, st);
            let ink = if here { brand::MAIN } else { brand::TEXT };
            self.c.text(ROW, r.x + 10.0, r.y + 3.0, ink, &trim(&bank.presets[i].name, 30));
            self.acts.push((r, Act::Patch(i)));
        }
        self.button(Rect::new(186.0, 512.0, 44.0, 34.0), "<", Act::List(-1), false);
        let well = Rect::new(236.0, 512.0, 300.0, 34.0);
        lemon::plug_well(self.c, well, 6.0);
        let title = format!("PAGE {}/{}   {} PATCHES", list + 1, pages, shown.len());
        self.c.text_center(Font::new(13).bold(), well, brand::MAIN, &title);
        self.button(Rect::new(542.0, 512.0, 44.0, 34.0), ">", Act::List(1), false);
        self.button(Rect::new(640.0, 512.0, 112.0, 34.0), "INIT", Act::Init, false);
    }
}

fn noise_at(v: i32) -> usize {
    crate::noise(v)
}
