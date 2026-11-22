// SPDX-License-Identifier: GPL-3.0-or-later
//! Rind FM's editor: every parameter of a DX7 voice on pages (the six operators, the voice's globals, the voices),
//! drawn in the ML Studio look. Knobs and switches are the host's parameters (the editor drags them); the tabs, the
//! voice list and the cartridge button are this extension's.
use std::path::Path;
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

use crate::banks::{self, Bank, Voice};
use crate::settings::{IDS, SETTINGS};
use crate::*;

const LABEL: Font = Font::new(10).bold();
const CAP: Font = Font::new(10);
const BIG: Font = Font::new(26).bold();
const ROW: Font = Font::new(13);
const TABS: [&str; 8] = ["OP 1", "OP 2", "OP 3", "OP 4", "OP 5", "OP 6", "GLOBAL", "VOICES"];
const GLOBAL_PAGE: usize = 6;
const VOICES_PAGE: usize = 7;

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Page(usize),
    Bank(i32),
    Voice(usize),
    Init,
    Cart,
}

struct St {
    page: usize,
    bank: usize,
    user: Vec<Bank>,
    acts: Vec<(Rect, Act)>,
    dialog: bool,
}

pub struct Panel {
    params: Arc<FxParams>,
    st: Mutex<St>,
}

/// What the voice says it is, from the saved state's tail: (bank, voice, name); the bank is 255 for an edited voice.
fn info(params: &FxParams) -> (usize, usize, String) {
    let raw = params.raw_copy();
    let at = params::state_len(&SPEC);
    match raw.get(at..at + 12) {
        Some(t) => (usize::from(t[0]), usize::from(t[1]), String::from_utf8_lossy(&t[2..12]).trim().to_string()),
        None => (255, 0, "INIT VOICE".into()),
    }
}

impl Panel {
    pub fn new(params: Arc<FxParams>) -> Arc<dyn Extension> {
        let bank = match info(&params).0 {
            b if b < banks::builtin() => b,
            _ => 0,
        };
        Arc::new(Panel { params, st: Mutex::new(St { page: 0, bank, user: Vec::new(), acts: Vec::new(), dialog: false }) })
    }

    fn set_info(&self, bank: usize, voice: usize, name: &str) {
        let mut raw = self.params.raw_copy();
        let at = params::state_len(&SPEC);
        raw.resize(at + 12, 0);
        raw[at] = bank.min(255) as u8;
        raw[at + 1] = voice.min(255) as u8;
        for (i, slot) in raw[at + 2..at + 12].iter_mut().enumerate() {
            *slot = name.as_bytes().get(i).copied().unwrap_or(b' ');
        }
        self.params.set_raw(raw);
    }

    /// Every setting to the voice's value: the sound hears it at once and the host is told.
    fn load(&self, v: &Voice, bank: usize, index: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        for k in 0..VOICE_BYTES {
            let s = &SETTINGS[k];
            vals.edit_plain(IDS[k], f32::from(i16::from(v[k])).clamp(s.range.0 as f32, s.range.1 as f32));
        }
        for n in 0..6 {
            vals.edit_plain(IDS[OP_ON + n], 1.0);
        }
        self.set_info(bank, index, &banks::name_of(v));
    }

    fn init(&self, ctx: Option<&Arc<dyn GuiContext>>) {
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        for (k, s) in SETTINGS.iter().enumerate() {
            if k != OUTPUT {
                vals.edit_plain(IDS[k], s.start as f32);
            }
        }
        self.set_info(255, 0, "INIT VOICE");
    }

    fn bank_len(st: &St, bank: usize) -> usize {
        if bank < banks::builtin() { 32 } else { st.user.get(bank - banks::builtin()).map_or(0, |b| b.voices.len()) }
    }

    fn bank_name(st: &St, bank: usize) -> String {
        if bank < banks::builtin() {
            banks::builtin_name(bank).to_string()
        } else {
            st.user.get(bank - banks::builtin()).map_or_else(String::new, |b| b.name.clone())
        }
    }

    fn voice(st: &St, bank: usize, index: usize) -> Option<Voice> {
        if bank < banks::builtin() {
            Some(banks::builtin_voice(bank, index))
        } else {
            st.user.get(bank - banks::builtin())?.voices.get(index).copied()
        }
    }
}

impl Extension for Panel {
    fn paint(&self, _: &mut Canvas, _: &dyn Model) {}

    fn accepts_files(&self) -> bool {
        true
    }

    /// A cartridge file: its voices become a bank of the list.
    fn file_dropped(&self, path: &Path) {
        let Ok(bytes) = std::fs::read(path) else { return };
        let name = path.file_stem().map_or_else(|| "Cartridge".to_string(), |s| s.to_string_lossy().into_owned());
        let Some(bank) = banks::parse_syx(&bytes, &name) else { return };
        if let Ok(mut st) = self.st.lock() {
            st.user.push(bank);
            st.bank = banks::builtin() + st.user.len() - 1;
            st.page = VOICES_PAGE;
        }
    }

    fn take_dialog(&self) -> bool {
        self.st.lock().map(|mut st| std::mem::take(&mut st.dialog)).unwrap_or(false)
    }

    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        let Ok(mut st) = self.st.lock() else { return false };
        let Some(&(_, act)) = st.acts.iter().rev().find(|(r, _)| r.contains(x, y)) else { return false };
        match act {
            Act::Page(p) => st.page = p,
            Act::Bank(d) => {
                let n = banks::builtin() + st.user.len();
                st.bank = (st.bank as i32 + d).rem_euclid(n as i32) as usize;
            }
            Act::Voice(i) => {
                if let Some(v) = Self::voice(&st, st.bank, i) {
                    self.load(&v, st.bank, i, ctx);
                }
            }
            Act::Init => self.init(ctx),
            Act::Cart => st.dialog = true,
        }
        true
    }

    fn render(&self, c: &mut Canvas, m: &dyn Model, state: &ViewState, _hint: &str) -> Option<Vec<Hit>> {
        let (page, bank) = {
            let st = self.st.lock().ok()?;
            (st.page, st.bank)
        };
        let loaded = info(&self.params);
        let mut ui = Ui { c, m, st: state, hits: Vec::new(), acts: Vec::new() };
        ui.c.begin_frame(None);
        ui.c.clear(plug::BG);
        for (i, name) in TABS.iter().enumerate() {
            ui.button(Rect::new(8.0 + 93.0 * i as f32, 8.0, 87.0, 28.0), name, Act::Page(i), page == i);
        }
        let algorithm = m.plain(IDS[ALGORITHM]).round() as i32 + 1;
        ui.lcd(&loaded, algorithm, m.plain(IDS[FEEDBACK]).round() as i32);
        match page {
            0..=5 => ui.operator(page + 1),
            GLOBAL_PAGE => ui.global(),
            _ => {
                let st = self.st.lock().ok()?;
                ui.voices(&st, bank, &loaded);
            }
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
        let on = self.m.plain(id).round() as i32 != 0;
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), i32::from(!on));
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    /// One position of a selector setting.
    fn option(&mut self, k: usize, value: i32, x: f32, cy: f32, label: &str) {
        let id = IDS[k];
        let on = self.m.plain(id).round() as i32 == value;
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), value);
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    fn lcd(&mut self, loaded: &(usize, usize, String), algorithm: i32, feedback: i32) {
        let r = Rect::new(8.0, 44.0, 744.0, 58.0);
        lemon::plug_well(self.c, r, 6.0);
        let glow = Color::rgb(0xb7e36a);
        let dim = Color::rgb(0x7f9a54);
        let (bank, voice, name) = loaded;
        let head = if *bank < 255 { format!("{:02}  {}", voice + 1, name) } else { name.clone() };
        self.c.text(BIG, r.x + 16.0, r.y + 6.0, glow, &head);
        let sub = if *bank < banks::builtin() {
            format!("BANK {:02}  {}", bank + 1, banks::builtin_name(*bank).to_uppercase())
        } else if *bank < 255 {
            "CARTRIDGE".to_string()
        } else {
            "EDITED VOICE".to_string()
        };
        self.c.text(CAP, r.x + 18.0, r.y + 38.0, dim, &sub);
        let alg = format!("ALG {algorithm:02}   FB {feedback}");
        self.c.text_right(Font::new(18).bold(), r.right() - 16.0, r.y + 18.0, glow, &alg);
    }

    fn operator(&mut self, n: usize) {
        let k = |i: usize| op_k(n, i);
        let on = self.m.plain(IDS[OP_ON + n - 1]).round() as i32 != 0;
        // Frequency.
        self.card(Rect::new(8.0, 110.0, 392.0, 126.0), "FREQUENCY");
        self.option(k(OP_MODE), 0, 26.0, 150.0, "RATIO");
        self.option(k(OP_MODE), 1, 26.0, 172.0, "FIXED");
        let read = freq_text(self.m.plain(IDS[k(OP_MODE)]).round() as i32, self.m.plain(IDS[k(OP_COARSE)]).round() as i32, self.m.plain(IDS[k(OP_FINE)]).round() as i32);
        self.c.text(Font::new(14).bold(), 22.0, 196.0, Color::rgb(0xb7e36a), &read);
        self.knob(k(OP_COARSE), 150.0, 178.0, 22.0, "COARSE");
        self.knob(k(OP_FINE), 250.0, 178.0, 22.0, "FINE");
        self.knob(k(OP_DET), 350.0, 178.0, 22.0, "DETUNE");
        // Level.
        self.card(Rect::new(408.0, 110.0, 344.0, 126.0), "LEVEL");
        self.knob(k(OP_LVL), 466.0, 178.0, 22.0, "OUTPUT");
        self.knob(k(OP_KVS), 566.0, 178.0, 22.0, "KEY VEL");
        self.knob(k(OP_AMS), 666.0, 178.0, 22.0, "A MOD");
        self.switch(OP_ON + n - 1, 690.0, 124.0, if on { "ON" } else { "OFF" });
        // Envelope.
        self.card(Rect::new(8.0, 244.0, 744.0, 192.0), "ENVELOPE");
        let rate = |m: &dyn Model, i: usize| m.plain(IDS[k(OP_R1 + i)]).round() as i32;
        let level = |m: &dyn Model, i: usize| m.plain(IDS[k(OP_L1 + i)]).round() as i32;
        let (rates, levels) = (std::array::from_fn(|i| rate(self.m, i)), std::array::from_fn(|i| level(self.m, i)));
        eg_graph(self.c, Rect::new(20.0, 270.0, 360.0, 150.0), rates, levels);
        for i in 0..4 {
            let cx = 436.0 + 88.0 * i as f32;
            self.knob(k(OP_R1 + i), cx, 304.0, 22.0, &format!("RATE {}", i + 1));
            self.knob(k(OP_L1 + i), cx, 392.0, 22.0, &format!("LEVEL {}", i + 1));
        }
        // Keyboard scaling.
        self.card(Rect::new(8.0, 444.0, 744.0, 108.0), "KEYBOARD SCALING");
        let ids = [(OP_BP, "BREAK POINT"), (OP_LD, "L DEPTH"), (OP_RD, "R DEPTH"), (OP_LC, "L CURVE"), (OP_RC, "R CURVE"), (OP_RS, "RATE SCALE")];
        for (j, (i, cap)) in ids.iter().enumerate() {
            self.knob(k(*i), 70.0 + 123.0 * j as f32, 502.0, 20.0, cap);
        }
    }

    fn global(&mut self) {
        let on: [bool; 6] = std::array::from_fn(|i| self.m.plain(IDS[OP_ON + i]).round() as i32 != 0);
        self.card(Rect::new(8.0, 110.0, 364.0, 330.0), "ALGORITHM");
        let algorithm = self.m.plain(IDS[ALGORITHM]).round() as usize;
        algo::draw(self.c, Rect::new(24.0, 138.0, 230.0, 286.0), algorithm, on);
        self.knob(ALGORITHM, 316.0, 196.0, 22.0, "ALGORITHM");
        self.knob(FEEDBACK, 316.0, 316.0, 22.0, "FEEDBACK");
        self.card(Rect::new(380.0, 110.0, 372.0, 126.0), "VOICE");
        self.knob(TRANSPOSE, 440.0, 178.0, 22.0, "TRANSPOSE");
        self.knob(OUTPUT, 530.0, 178.0, 22.0, "OUTPUT");
        self.switch(OSC_SYNC, 592.0, 150.0, "OSC KEY SYNC");
        for n in 0..6 {
            self.switch(OP_ON + n, 600.0 + 24.0 * n as f32 + if n > 2 { 6.0 } else { 0.0 }, 196.0, &(n + 1).to_string());
        }
        self.card(Rect::new(380.0, 244.0, 372.0, 196.0), "LFO");
        self.knob(LFO_WAVE, 440.0, 296.0, 22.0, "WAVE");
        self.knob(LFO_SPEED, 530.0, 296.0, 22.0, "SPEED");
        self.knob(LFO_DELAY, 620.0, 296.0, 22.0, "DELAY");
        self.knob(LFO_PMD, 440.0, 386.0, 22.0, "PITCH DEPTH");
        self.knob(LFO_AMD, 530.0, 386.0, 22.0, "AMP DEPTH");
        self.knob(PITCH_MOD_SENS, 620.0, 386.0, 22.0, "PITCH SENS");
        self.switch(LFO_SYNC, 676.0, 296.0, "KEY SYNC");
        self.card(Rect::new(8.0, 448.0, 744.0, 104.0), "PITCH ENVELOPE");
        for i in 0..4 {
            self.knob(PEG_R + i, 60.0 + 92.0 * i as f32, 506.0, 18.0, &format!("RATE {}", i + 1));
            self.knob(PEG_L + i, 440.0 + 92.0 * i as f32, 506.0, 18.0, &format!("LEVEL {}", i + 1));
        }
    }

    fn voices(&mut self, st: &St, bank: usize, loaded: &(usize, usize, String)) {
        self.button(Rect::new(8.0, 112.0, 44.0, 34.0), "<", Act::Bank(-1), false);
        let well = Rect::new(58.0, 112.0, 410.0, 34.0);
        lemon::plug_well(self.c, well, 6.0);
        let n = banks::builtin() + st.user.len();
        let title = format!("{:02}/{:02}   {}", bank + 1, n, Panel::bank_name(st, bank));
        self.c.text_center(Font::new(14).bold(), well, brand::MAIN, &title);
        self.button(Rect::new(474.0, 112.0, 44.0, 34.0), ">", Act::Bank(1), false);
        self.button(Rect::new(530.0, 112.0, 100.0, 34.0), "INIT", Act::Init, false);
        self.button(Rect::new(638.0, 112.0, 114.0, 34.0), "LOAD CART", Act::Cart, false);
        for i in 0..Panel::bank_len(st, bank).min(32) {
            let (col, row) = (i / 16, i % 16);
            let r = Rect::new(8.0 + 378.0 * col as f32, 156.0 + 24.5 * row as f32, 366.0, 23.0);
            let here = loaded.0 == bank && loaded.1 == i;
            let hover = self.hover(r);
            lemon::field(self.c, r, State { hover, on: here, ..State::NORMAL });
            let name = Panel::voice(st, bank, i).map(|v| banks::name_of(&v)).unwrap_or_default();
            let ink = if here { brand::MAIN } else { brand::TEXT };
            self.c.text(ROW, r.x + 10.0, r.y + 3.0, plug::DIM, &format!("{:02}", i + 1));
            self.c.text(ROW, r.x + 40.0, r.y + 3.0, ink, &name);
            self.acts.push((r, Act::Voice(i)));
        }
    }
}

/// The frequency an operator plays: its ratio to the note, or fixed Hz.
fn freq_text(mode: i32, coarse: i32, fine: i32) -> String {
    if mode == 0 {
        let base = if coarse == 0 { 0.5 } else { f64::from(coarse) };
        format!("x {:.2}", base * (1.0 + f64::from(fine) / 100.0))
    } else {
        let hz = 10f64.powi(coarse & 3) * (1.0 + 8.772 * f64::from(fine) / 99.0);
        format!("{hz:.1} Hz")
    }
}

/// An envelope: rise from the last level to L1, L2, L3 at the rates, hold until the key goes up, fall to L4.
fn eg_graph(c: &mut Canvas, r: Rect, rate: [i32; 4], level: [i32; 4]) {
    lemon::plug_well(c, r, 6.0);
    let inner = r.inset(10.0);
    let seg = |from: i32, to: i32, rate: i32| 4.0 + ((to - from).abs() as f32 / 99.0) * (99 - rate) as f32 * 0.9 + (99 - rate) as f32 * 0.1;
    let t = [seg(level[3], level[0], rate[0]), seg(level[0], level[1], rate[1]), seg(level[1], level[2], rate[2])];
    let fall = seg(level[2], level[3], rate[3]);
    let hold = 0.2 * (t[0] + t[1] + t[2] + fall);
    let total = t[0] + t[1] + t[2] + hold + fall;
    let x = |at: f32| inner.x + inner.w * at / total;
    let y = |l: i32| inner.bottom() - inner.h * l as f32 / 99.0;
    let xs = [0.0, t[0], t[0] + t[1], t[0] + t[1] + t[2], t[0] + t[1] + t[2] + hold, total];
    let ls = [level[3], level[0], level[1], level[2], level[2], level[3]];
    let pts: Vec<(f32, f32)> = xs.iter().zip(ls).map(|(&a, l)| (x(a), y(l))).collect();
    let mut fill = pts.clone();
    fill.push((x(total), inner.bottom()));
    fill.push((x(0.0), inner.bottom()));
    c.fill_polygon(&fill, brand::MAIN.with_alpha(40));
    c.line(x(xs[3]), inner.y, x(xs[3]), inner.bottom(), 1.0, Color::rgb(0x4a4a4a));
    c.polyline(&pts, 2.0, brand::MAIN);
    for p in &pts[1..4] {
        c.fill_circle(p.0, p.1, 3.0, Color::rgb(0xe6e6e6));
    }
}
