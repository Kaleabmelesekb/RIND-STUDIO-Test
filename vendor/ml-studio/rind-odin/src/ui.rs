// SPDX-License-Identifier: GPL-3.0-or-later
//! Rind Odin's editor: the synth on seven pages (the oscillators, the filters, the envelopes and LFOs, the effects, the
//! modulation matrix, the arpeggiator and voice, the patches), drawn in the ML Studio look. Knobs and switches are the
//! host's parameters (the editor drags them); the tabs, the drop-downs and the patch list are this extension's.
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ml_small_fx::editor::Extension;
use ml_small_fx::params::{self, FxParams};
use ml_small_fx::view::{Hit, Model, Target, ViewState};
use nih_plug::prelude::GuiContext;
use os_plugkit::vals::Vals;
use os_theme::design::lemon as brand;
use os_ui::kit::lemon::{self, plug};
use os_ui::kit::State;
use os_ui::{Canvas, Color, Font, Rect};

use crate::patch::{self, Patch};
use crate::presets;
use crate::settings::*;
use crate::{Key, NONE, Source};

const LABEL: Font = Font::new(10).bold();
const CAP: Font = Font::new(10);
const ROW: Font = Font::new(13);
const TABS: [&str; 7] = ["OSC", "FILTER", "ENV/LFO", "FX", "MOD", "ARP/VOICE", "PATCHES"];
const PATCHES_PAGE: usize = 6;
const PER_PAGE: usize = 32;
/// How long the display shows what a file did.
const NOTE_FOR: Duration = Duration::from_secs(4);
/// The most patches kept from files.
const MAX_USER: usize = 256;
/// The biggest patch file read (Odin 2's own are 15 to 45 KB).
const MAX_FILE: u64 = 4 << 20;

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Page(usize),
    /// The category list: 0 is all, then the categories, then the patches from files.
    Category(usize),
    Patch(usize),
    /// A patch opened from a file.
    User(usize),
    Step(i32),
    List(i32),
    Init,
    Open,
    /// A drop-down's field: its choices open under it.
    Menu(usize),
    /// A choice picked in the open drop-down.
    Choose(usize, i32),
    /// The open drop-down's next (or previous) page of choices.
    MenuPage(i32),
    /// A click on the open drop-down's own panel, between its choices.
    Nothing,
    /// A click away from the open drop-down.
    CloseMenu,
}

/// An open drop-down: the setting, the field it came from and the page of its choices that shows.
#[derive(Clone, Copy)]
struct Menu {
    k: usize,
    anchor: Rect,
    page: usize,
}

const EDITOR: (f32, f32) = (760.0, 560.0);
const CHOICE_H: f32 = 24.0;
const MENU_ROWS: usize = 14;

/// How a drop-down of `n` choices lays out: its columns, rows and the choices a page holds.
fn menu_shape(n: usize) -> (usize, usize, usize) {
    let cols = match n {
        0..=14 => 1,
        15..=28 => 2,
        _ => 3,
    };
    let rows = n.div_ceil(cols).clamp(1, MENU_ROWS);
    (cols, rows, cols * rows)
}

/// The number of choices of setting `k` and the number of pages they take.
fn menu_size(k: usize) -> (usize, usize) {
    let (lo, hi) = SETTINGS[k].range;
    let n = (hi - lo + 1).max(0) as usize;
    (n, n.div_ceil(menu_shape(n).2).max(1))
}

/// The patch the editor says is loaded.
#[derive(Clone, Copy, PartialEq)]
enum Loaded {
    None,
    Factory(usize),
    File,
}

struct User {
    name: String,
    data: Vec<u8>,
}

struct St {
    page: usize,
    category: usize,
    list: usize,
    acts: Vec<(Rect, Act)>,
    /// The drop-down that is open.
    menu: Option<Menu>,
    /// The patches of the files opened.
    user: Vec<User>,
    /// The editor asked for a file to open.
    dialog: bool,
    /// What the last file did, for the display.
    note: Option<(String, Instant)>,
    /// The host's context, from the last click: a patch opened from a file loads with it.
    ctx: Option<Arc<dyn GuiContext>>,
    /// The patch the saved state names, read.
    cache: Option<(Key, Option<Patch>)>,
}

pub struct Panel {
    params: Arc<FxParams>,
    st: Mutex<St>,
}

fn imported() -> usize {
    presets::bank().categories.len() + 1
}

fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}

impl Panel {
    pub fn new(params: Arc<FxParams>) -> Arc<dyn Extension> {
        let category = match crate::source_of(&params.raw_copy()) {
            Source::Factory(i) => presets::bank().patches[i].category + 1,
            _ => 0,
        };
        let st = St {
            page: 0,
            category,
            list: 0,
            acts: Vec::new(),
            menu: None,
            user: Vec::new(),
            dialog: false,
            note: None,
            ctx: None,
            cache: None,
        };
        Arc::new(Panel { params, st: Mutex::new(st) })
    }

    fn with(&self, f: impl FnOnce(&mut St)) {
        if let Ok(mut st) = self.st.lock() {
            f(&mut st);
        }
    }

    /// The saved state's patch: the factory patch's place, or the file that follows it.
    fn set_tail(&self, source: &Source) {
        let mut raw = self.params.raw_copy();
        let at = params::state_len(&crate::SPEC);
        raw.resize(at, 0);
        match source {
            Source::Factory(i) => raw.extend_from_slice(&((*i).min(0xFFFE) as u16).to_le_bytes()),
            Source::File(b) => {
                raw.extend_from_slice(&NONE.to_le_bytes());
                raw.extend_from_slice(b);
            }
            Source::Init => raw.extend_from_slice(&NONE.to_le_bytes()),
        }
        self.params.set_raw(raw);
    }

    /// Every setting to the patch's value: the sound hears it at once and the host is told.
    fn load_values(&self, values: &[i32], ctx: Option<&Arc<dyn GuiContext>>) {
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        for (k, &v) in values.iter().enumerate().take(COUNT) {
            vals.edit_plain(SETTINGS[k].id, v as f32);
        }
    }

    /// Loads a patch: the engine builds it on its own thread (the saved state names it first, so that the sound keeps
    /// playing the old one meanwhile), and the settings are the patch's.
    fn load(&self, source: Source, values: &[i32], ctx: Option<&Arc<dyn GuiContext>>) {
        self.set_tail(&source);
        self.load_values(values, ctx);
    }

    fn load_factory(&self, index: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let Some(p) = presets::bank().patches.get(index) else { return };
        if let Ok(patch) = patch::parse(p.data) {
            self.load(Source::Factory(index), &patch.values, ctx);
        }
    }

    fn load_user(&self, index: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let data = self.st.lock().ok().and_then(|st| st.user.get(index).map(|u| u.data.clone()));
        let Some(data) = data else { return };
        if let Ok(patch) = patch::parse(&data) {
            self.load(Source::File(data), &patch.values, ctx);
        }
    }

    fn init(&self, ctx: Option<&Arc<dyn GuiContext>>) {
        let values: Vec<i32> = SETTINGS.iter().map(|s| s.start).collect();
        self.load(Source::Init, &values, ctx);
    }

    /// Sets setting `k` (an editor edit).
    fn set(&self, k: usize, v: i32, ctx: Option<&Arc<dyn GuiContext>>) {
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        vals.edit_plain(SETTINGS[k].id, v as f32);
    }

    /// The patches the category list shows, as places in the bank (or in the opened ones, for the last row).
    fn shown(category: usize, user: usize) -> Vec<usize> {
        if category == imported() {
            return (0..user).collect();
        }
        let b = presets::bank();
        (0..b.patches.len()).filter(|&i| category == 0 || b.patches[i].category + 1 == category).collect()
    }
}

impl Extension for Panel {
    fn paint(&self, _: &mut Canvas, _: &dyn Model) {}

    fn accepts_files(&self) -> bool {
        true
    }

    /// An Odin 2 patch (a `.odin` file, from anywhere): it joins the list of patches from files and loads.
    fn file_dropped(&self, path: &Path) {
        let read = || -> Result<(Vec<u8>, Patch), &'static str> {
            let len = std::fs::metadata(path).map_err(|_| "CAN'T READ THE FILE")?.len();
            if len > MAX_FILE {
                return Err("TOO BIG FOR A PATCH");
            }
            let data = std::fs::read(path).map_err(|_| "CAN'T READ THE FILE")?;
            let patch = patch::parse(&data)?;
            Ok((data, patch))
        };
        let (data, patch) = match read() {
            Ok(r) => r,
            Err(e) => {
                self.with(|st| st.note = Some((e.to_string(), Instant::now())));
                return;
            }
        };
        let name = if patch.name.trim().is_empty() || patch.name == "init_patch" {
            path.file_stem().map_or_else(|| "PATCH".to_string(), |s| s.to_string_lossy().into_owned())
        } else {
            patch.name.clone()
        };
        let (index, ctx) = {
            let Ok(mut st) = self.st.lock() else { return };
            st.user.push(User { name, data });
            if st.user.len() > MAX_USER {
                st.user.remove(0);
            }
            let index = st.user.len() - 1;
            st.page = PATCHES_PAGE;
            st.category = imported();
            st.list = index / PER_PAGE;
            st.note = Some(("PATCH OPENED".to_string(), Instant::now()));
            (index, st.ctx.clone())
        };
        self.load_user(index, ctx.as_ref());
    }

    fn take_dialog(&self) -> bool {
        self.st.lock().map(|mut st| std::mem::take(&mut st.dialog)).unwrap_or(false)
    }

    fn dialog_filter(&self) -> (&'static str, &'static str, &'static [&'static str]) {
        ("Open an Odin 2 patch", "Odin 2 patches", &["odin"])
    }

    fn wheel(&self, _x: f32, _y: f32, notches: f32) -> bool {
        let Ok(mut st) = self.st.lock() else { return false };
        let Some(m) = &mut st.menu else { return false };
        let pages = menu_size(m.k).1 as i32;
        if pages > 1 && notches != 0.0 {
            let d = if notches < 0.0 { 1 } else { -1 };
            m.page = (m.page as i32 + d).rem_euclid(pages) as usize;
        }
        true
    }

    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        let act = {
            let Ok(mut st) = self.st.lock() else { return false };
            if ctx.is_some() {
                st.ctx = ctx.cloned();
            }
            let Some(&(field, act)) = st.acts.iter().rev().find(|(r, _)| r.contains(x, y)) else { return false };
            if let Act::Menu(k) = act {
                let (lo, _) = SETTINGS[k].range;
                let per = menu_shape(menu_size(k).0).2;
                let page = (self.params.v(k) - lo).max(0) as usize / per;
                st.menu = Some(Menu { k, anchor: field, page });
            }
            act
        };
        match act {
            Act::Page(p) => self.with(|st| st.page = p),
            Act::Category(c) => self.with(|st| {
                st.category = c;
                st.list = 0;
            }),
            Act::Patch(i) => self.load_factory(i, ctx),
            Act::User(i) => self.load_user(i, ctx),
            Act::Step(d) => {
                let n = presets::bank().patches.len() as i32;
                if n > 0 {
                    let now = match crate::source_of(&self.params.raw_copy()) {
                        Source::Factory(i) => i as i32,
                        _ => {
                            if d > 0 {
                                -1
                            } else {
                                0
                            }
                        }
                    };
                    let next = (now + d).rem_euclid(n) as usize;
                    self.load_factory(next, ctx);
                    self.with(|st| st.category = presets::bank().patches[next].category + 1);
                }
            }
            Act::List(d) => self.with(|st| {
                let pages = Self::shown(st.category, st.user.len()).len().div_ceil(PER_PAGE).max(1) as i32;
                st.list = (st.list as i32 + d).rem_euclid(pages) as usize;
            }),
            Act::Init => self.init(ctx),
            Act::Open => self.with(|st| st.dialog = true),
            Act::Menu(_) | Act::Nothing => {}
            Act::Choose(k, v) => {
                self.with(|st| st.menu = None);
                self.set(k, v, ctx);
            }
            Act::MenuPage(d) => self.with(|st| {
                if let Some(m) = &mut st.menu {
                    let pages = menu_size(m.k).1 as i32;
                    m.page = (m.page as i32 + d).rem_euclid(pages) as usize;
                }
            }),
            Act::CloseMenu => self.with(|st| st.menu = None),
        }
        true
    }

    fn render(&self, c: &mut Canvas, m: &dyn Model, state: &ViewState, _hint: &str) -> Option<Vec<Hit>> {
        let (page, category, list, user_count, names, note, menu) = {
            let st = self.st.lock().ok()?;
            let names = if st.page == PATCHES_PAGE && st.category == imported() {
                st.user.iter().map(|p| p.name.clone()).collect()
            } else {
                Vec::new()
            };
            let note = st.note.as_ref().filter(|(_, t)| t.elapsed() < NOTE_FOR).map(|(s, _)| s.clone());
            (st.page, st.category, st.list, st.user.len(), names, note, st.menu)
        };
        // The patch the saved state names, read once.
        let source = crate::source_of(&self.params.raw_copy());
        let key = source.key();
        let patch = {
            let mut st = self.st.lock().ok()?;
            if st.cache.as_ref().is_none_or(|(k, _)| *k != key) {
                let data: Option<&[u8]> = match &source {
                    Source::Init => None,
                    Source::Factory(i) => presets::bank().patches.get(*i).map(|p| p.data),
                    Source::File(b) => Some(b),
                };
                st.cache = Some((key, data.and_then(|d| patch::parse(d).ok())));
            }
            st.cache.as_ref().and_then(|(_, p)| p.clone())
        };
        let loaded = match &source {
            Source::Factory(i) => Loaded::Factory(*i),
            Source::File(_) => Loaded::File,
            Source::Init => Loaded::None,
        };
        let edited = match &patch {
            Some(p) => p.values.iter().enumerate().take(COUNT).any(|(k, &v)| m.plain(SETTINGS[k].id).round() as i32 != v),
            None => SETTINGS.iter().any(|s| m.plain(s.id).round() as i32 != s.start),
        };
        // The display: the patch, and what it is or that it was changed.
        let bank = presets::bank();
        let (head, mut sub) = match &source {
            Source::Factory(i) => {
                let p = &bank.patches[*i];
                let cat = bank.categories.get(p.category).copied().unwrap_or("");
                (p.name.to_string(), format!("{}{}", cat.to_uppercase(), if edited { "  EDITED" } else { "" }))
            }
            Source::File(_) => {
                let name = patch.as_ref().map(|p| p.name.clone()).filter(|n| !n.trim().is_empty());
                (name.unwrap_or_else(|| "PATCH".to_string()), format!("FILE{}", if edited { "  EDITED" } else { "" }))
            }
            Source::Init => ("INIT".to_string(), if edited { "EDITED PATCH" } else { "NEW PATCH" }.to_string()),
        };
        if let Some(note) = note {
            sub = note;
        }
        let mut ui = Ui { c, m, st: state, hits: Vec::new(), acts: Vec::new() };
        ui.c.begin_frame(None);
        ui.c.clear(plug::BG);
        for (i, name) in TABS.iter().enumerate() {
            ui.button(Rect::new(8.0 + 106.0 * i as f32, 8.0, 100.0, 28.0), name, Act::Page(i), page == i);
        }
        ui.lcd(&trim(&head, 22), &trim(&sub, 40));
        ui.master();
        match page {
            0 => ui.oscillators(),
            1 => ui.filters(),
            2 => ui.envelopes(),
            3 => ui.effects(),
            4 => ui.matrix(),
            5 => ui.arp(),
            _ => ui.patches(category, list, loaded, &names, user_count),
        }
        if let Some(menu) = menu {
            ui.menu(menu);
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

    fn value(&self, k: usize) -> i32 {
        self.m.plain(SETTINGS[k].id).round() as i32
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
        let id = SETTINGS[k].id;
        let target = Target::Knob(id.to_string());
        let hr = Rect::new(cx - r - 4.0, cy - r - 4.0, 2.0 * r + 8.0, 2.0 * r + 8.0);
        let st = self.state(hr, &target);
        let bipolar = SETTINGS[k].range.0 < 0 && SETTINGS[k].range.1 > 0;
        lemon::dial(self.c, cx, cy, r, 3.2, self.m.norm(id), bipolar, brand::MAIN, st);
        self.c.text_center(CAP, Rect::new(cx - 44.0, cy - r - 17.0, 88.0, 13.0), plug::DIM, caption);
        let text = self.m.text(id);
        self.c.text_center(LABEL, Rect::new(cx - 44.0, cy + r + 4.0, 88.0, 13.0), brand::MAIN, &text);
        self.hits.push(Hit { r: hr, t: target });
    }

    /// A small dial with its value under it, no caption (the matrix has headings).
    fn mini(&mut self, k: usize, cx: f32, cy: f32, r: f32) {
        let id = SETTINGS[k].id;
        let target = Target::Knob(id.to_string());
        let hr = Rect::new(cx - r - 4.0, cy - r - 4.0, 2.0 * r + 8.0, 2.0 * r + 8.0);
        let st = self.state(hr, &target);
        lemon::dial(self.c, cx, cy, r, 2.6, self.m.norm(id), true, brand::MAIN, st);
        let text = self.m.text(id);
        self.c.text_center(CAP, Rect::new(cx - 30.0, cy + r + 2.0, 60.0, 12.0), brand::MAIN, &text);
        self.hits.push(Hit { r: hr, t: target });
    }

    /// An on/off setting (0 or 1) as a switch with its name beside it.
    fn switch(&mut self, k: usize, x: f32, cy: f32, label: &str) {
        let id = SETTINGS[k].id;
        let on = self.value(k) >= 1;
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), i32::from(!on));
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    /// One position of a selector setting: `value` is what it stores, `on` whether the setting is at it now.
    fn option(&mut self, k: usize, value: i32, on: bool, x: f32, cy: f32, label: &str) {
        let id = SETTINGS[k].id;
        let w = self.c.text_width(LABEL, label);
        let r = Rect::new(x - 8.0, cy - 11.0, 24.0 + w, 22.0);
        let target = Target::Choice(id.to_string(), value);
        let st = self.state(r, &target);
        lemon::toggle(self.c, x, cy, on, st);
        self.c.text(LABEL, x + 12.0, cy - 6.0, if on { brand::MAIN } else { plug::DIM }, label);
        self.hits.push(Hit { r, t: target });
    }

    /// A selector setting as a field with its value in it; a tap opens a drop-down of the choices under it.
    fn dropdown(&mut self, k: usize, r: Rect) {
        let st = State { hover: self.hover(r), ..State::NORMAL };
        lemon::field(self.c, r, st);
        let text = self.m.text(SETTINGS[k].id);
        let off = text == "Off" || text == "OFF";
        let name = trim(&text, ((r.w - 30.0) / 6.0) as usize);
        self.c.text_center(LABEL, Rect::new(r.x + 6.0, r.y, r.w - 26.0, r.h), if off { plug::DIM } else { brand::MAIN }, &name);
        let (cx, cy) = (r.right() - 12.0, r.y + r.h / 2.0);
        self.c.fill_polygon(&[(cx - 4.0, cy - 2.0), (cx + 4.0, cy - 2.0), (cx, cy + 3.0)], plug::DIM);
        self.acts.push((r, Act::Menu(k)));
    }

    /// The open drop-down, over the editor: a click away from it closes it, a click on a choice picks it.
    fn menu(&mut self, menu: Menu) {
        let k = menu.k;
        let (lo, _) = SETTINGS[k].range;
        let (n, pages) = menu_size(k);
        let (cols, rows, per) = menu_shape(n);
        let col_w = if cols == 1 { menu.anchor.w.max(150.0) } else { 176.0 };
        let (w, foot) = (cols as f32 * col_w + 12.0, if pages > 1 { 32.0 } else { 0.0 });
        let h = rows as f32 * CHOICE_H + 12.0 + foot;
        let x = menu.anchor.x.min(EDITOR.0 - 8.0 - w).max(8.0);
        let below = menu.anchor.y + menu.anchor.h + 2.0;
        let y = if below + h <= EDITOR.1 - 8.0 { below } else { (menu.anchor.y - h - 2.0).max(8.0) };
        self.hits.clear();
        self.acts.clear();
        self.c.fill_rect(Rect::new(0.0, 0.0, EDITOR.0, EDITOR.1), plug::BG.with_alpha(150));
        self.acts.push((Rect::new(0.0, 0.0, EDITOR.0, EDITOR.1), Act::CloseMenu));
        let panel = Rect::new(x, y, w, h);
        lemon::plug_panel(self.c, panel);
        self.acts.push((panel, Act::Nothing));
        let now = self.value(k);
        let page = menu.page.min(pages - 1);
        for slot in 0..per {
            let i = page * per + slot;
            if i >= n {
                break;
            }
            let v = lo + i as i32;
            let (col, row) = (slot / rows, slot % rows);
            let r = Rect::new(x + 6.0 + col as f32 * col_w, y + 6.0 + row as f32 * CHOICE_H, col_w, CHOICE_H);
            let on = v == now;
            if on || self.hover(r) {
                self.c.fill_rrect(r.inset(1.0), 5.0, if on { Color::rgb(0x4a4a32) } else { Color::rgb(0x424242) });
            }
            let label = trim(&crate::text(k, v), ((col_w - 16.0) / 6.0) as usize);
            self.c.text(LABEL, r.x + 8.0, r.y + 7.0, if on { brand::MAIN } else { Color::rgb(0xdcdcdc) }, &label);
            self.acts.push((r, Act::Choose(k, v)));
        }
        if pages > 1 {
            let by = y + h - foot + 2.0;
            self.button(Rect::new(x + 8.0, by, 56.0, 26.0), "<", Act::MenuPage(-1), false);
            self.button(Rect::new(x + w - 64.0, by, 56.0, 26.0), ">", Act::MenuPage(1), false);
            let label = format!("{} - {} OF {}", page * per + 1, ((page + 1) * per).min(n), n);
            self.c.text_center(LABEL, Rect::new(x + 70.0, by, w - 140.0, 26.0), plug::DIM, &label);
        }
    }

    fn lcd(&mut self, head: &str, sub: &str) {
        let r = Rect::new(8.0, 44.0, 300.0, 60.0);
        lemon::plug_well(self.c, r, 6.0);
        let glow = Color::rgb(0xb7e36a);
        let dim = Color::rgb(0x7f9a54);
        self.c.text(Font::new(20).bold(), r.x + 14.0, r.y + 8.0, glow, head);
        self.c.text(CAP, r.x + 16.0, r.y + 38.0, dim, sub);
        self.button(Rect::new(314.0, 44.0, 32.0, 60.0), "<", Act::Step(-1), false);
        self.button(Rect::new(350.0, 44.0, 32.0, 60.0), ">", Act::Step(1), false);
    }

    /// The top bar on every page: the volume, the glide, the arpeggiator and the voice mode.
    fn master(&mut self) {
        lemon::plug_panel(self.c, Rect::new(388.0, 44.0, 364.0, 60.0));
        self.knob(MASTER, 424.0, 76.0, 11.0, "MASTER");
        self.knob(GLIDE, 486.0, 76.0, 11.0, "GLIDE");
        self.switch(ARP_ON, 540.0, 76.0, "ARP");
        self.dropdown(LEGATO, Rect::new(612.0, 62.0, 130.0, 26.0));
    }

    /// The knobs of a row, `per` to a row from (`x`, `y`) at `step` apart.
    fn knobs(&mut self, list: &[(usize, &str)], x: f32, y: f32, step: f32, per: usize, row: f32, r: f32) {
        for (i, (k, cap)) in list.iter().enumerate() {
            self.knob(*k, x + step * (i % per) as f32, y + row * (i / per) as f32, r, cap);
        }
    }

    fn oscillators(&mut self) {
        for n in 0..3 {
            let x = 8.0 + 252.0 * n as f32;
            self.card(Rect::new(x, 112.0, 240.0, 440.0), &format!("OSC {}", n + 1));
            self.dropdown(OSC_TYPE[n], Rect::new(x + 10.0, 136.0, 220.0, 24.0));
            let kind = T_OSC_TYPES.get(self.value(OSC_TYPE[n]).max(0) as usize).map_or(0, |t| t.0);
            if kind == 1 {
                self.c.text_center(ROW, Rect::new(x, 300.0, 240.0, 20.0), plug::DIM, "OFF");
                continue;
            }
            let cx = [x + 44.0, x + 120.0, x + 196.0];
            let common = [
                (OSC_OCT[n], "OCTAVE", 0),
                (OSC_SEMI[n], "SEMI", 1),
                (OSC_FINE[n], "FINE", 2),
                (OSC_VOL[n], "VOLUME", 0),
                (OSC_DRIFT[n], "DRIFT", 1),
                (OSC_DETUNE[n], "DETUNE", 2),
            ];
            for (i, (k, cap, col)) in common.iter().enumerate() {
                self.knob(*k, cx[*col], 202.0 + 76.0 * (i / 3) as f32, 16.0, cap);
            }
            // What the oscillator type has of its own.
            let own: Vec<(usize, &str)> = match kind {
                2 => vec![(OSC_PULSEWIDTH[n], "PULSE WIDTH")],
                3 => vec![(OSC_POSITION[n], "POSITION"), (OSC_POS_MOD[n], "POS MOD")],
                4 => vec![(OSC_MULTI_POSITION[n], "POSITION"), (OSC_SPREAD[n], "SPREAD")],
                5 => vec![(OSC_VEC_X[n], "X"), (OSC_VEC_Y[n], "Y")],
                6 => vec![(OSC_ARP_SPEED[n], "ARP SPEED"), (OSC_STEP_1[n], "STEP 1"), (OSC_STEP_2[n], "STEP 2"), (OSC_STEP_3[n], "STEP 3")],
                7 | 8 => vec![(OSC_FM[n], "AMOUNT")],
                15 => vec![(OSC_LP[n], "LOW PASS"), (OSC_HP[n], "HIGH PASS")],
                _ => Vec::new(),
            };
            match kind {
                2 => {
                    let wave = self.value(OSC_ANALOG_WAVE[n]);
                    for (i, (v, name)) in T_ANALOG.iter().enumerate() {
                        self.option(OSC_ANALOG_WAVE[n], *v, wave == *v, x + 20.0 + 108.0 * (i % 2) as f32, 372.0 + 24.0 * (i / 2) as f32, name);
                    }
                }
                3 | 4 => self.dropdown(OSC_WAVETABLE[n], Rect::new(x + 10.0, 336.0, 220.0, 24.0)),
                20..=22 => self.c.text_center(CAP, Rect::new(x, 340.0, 240.0, 14.0), plug::DIM, "THE WAVE IS THE PATCH'S OWN"),
                _ => {}
            }
            let row = if matches!(kind, 3 | 4) { 404.0 } else if kind == 2 { 450.0 } else { 398.0 };
            self.knobs(&own, cx[0], row, 76.0, 3, 76.0, 16.0);
            match kind {
                6 => {
                    self.switch(OSC_ARP_ON[n], x + 20.0, 342.0, "ARP");
                    self.switch(OSC_STEP_3_ON[n], x + 76.0, 342.0, "STEP 3");
                    self.switch(OSC_CHIPNOISE[n], x + 150.0, 342.0, "NOISE");
                }
                7 => self.switch(OSC_EXP_FM[n], x + 20.0, 342.0, "EXPONENTIAL FM"),
                _ => {}
            }
            self.switch(OSC_RESET[n], x + 20.0, 534.0, "RESET");
            if n == 1 {
                self.switch(OSC_SYNC_2, x + 120.0, 534.0, "SYNC");
            } else if n == 2 {
                self.switch(OSC_SYNC_3, x + 120.0, 534.0, "SYNC");
            }
        }
    }

    fn filters(&mut self) {
        for n in 0..3 {
            let x = 8.0 + 252.0 * n as f32;
            self.card(Rect::new(x, 112.0, 240.0, 440.0), &format!("FILTER {}", n + 1));
            self.dropdown(FIL_TYPE[n], Rect::new(x + 10.0, 136.0, 220.0, 24.0));
            let kind = T_FIL_TYPES.get(self.value(FIL_TYPE[n]).max(0) as usize).map_or(0, |t| t.0);
            if kind == 1 {
                self.c.text_center(ROW, Rect::new(x, 300.0, 240.0, 20.0), plug::DIM, "OFF");
            } else {
                let mut list = vec![
                    (FIL_FREQ[n], "FREQUENCY"),
                    (FIL_RES[n], "RESONANCE"),
                    (FIL_GAIN[n], "GAIN"),
                    (FIL_ENV[n], "ENV AMOUNT"),
                    (FIL_VEL[n], "VELOCITY"),
                    (FIL_KBD[n], "KEYTRACK"),
                    (FIL_SATURATION[n], "SATURATION"),
                ];
                match kind {
                    15 => list.push((FIL_SEM_TRANSITION[n], "LP-BP-HP")),
                    35 => list.push((FIL_FORMANT_TRANSITION[n], "VOWEL")),
                    40 => list.push((FIL_RING_MOD_AMOUNT[n], "RING AMOUNT")),
                    _ => {}
                }
                self.knobs(&list, x + 44.0, 202.0, 76.0, 3, 80.0, 16.0);
            }
            // What feeds the filter and where it goes.
            self.c.text(LABEL, x + 14.0, 424.0, plug::DIM, "INPUT");
            if n < 2 {
                self.switch(FIL_OSC1[n], x + 20.0, 452.0, "OSC 1");
                self.switch(FIL_OSC2[n], x + 92.0, 452.0, "OSC 2");
                self.switch(FIL_OSC3[n], x + 164.0, 452.0, "OSC 3");
                if n == 1 {
                    self.switch(FIL_FIL1_2, x + 20.0, 480.0, "FILTER 1");
                }
                self.switch(FIL_TO_AMP[n], x + 120.0, 508.0, "TO AMP");
            } else {
                self.c.text(CAP, x + 14.0, 448.0, plug::DIM, "THE OSCILLATORS AND");
                self.c.text(CAP, x + 14.0, 462.0, plug::DIM, "FILTERS 1 AND 2");
            }
        }
    }

    fn envelopes(&mut self) {
        let names = ["AMP ENVELOPE", "FILTER ENVELOPE", "MOD ENVELOPE", "GLOBAL ENVELOPE"];
        for n in 0..4 {
            let y = 112.0 + 111.0 * n as f32;
            self.card(Rect::new(8.0, y, 364.0, 106.0), names[n]);
            let list = [(ENV_ATTACK[n], "ATTACK"), (ENV_DECAY[n], "DECAY"), (ENV_SUSTAIN[n], "SUSTAIN"), (ENV_RELEASE[n], "RELEASE")];
            self.knobs(&list, 42.0, y + 62.0, 66.0, 4, 0.0, 14.0);
            self.switch(ENV_LOOP[n], 312.0, y + 62.0, "LOOP");
        }
        let names = ["LFO 1", "LFO 2", "LFO 3", "GLOBAL LFO"];
        for n in 0..4 {
            let y = 112.0 + 111.0 * n as f32;
            self.card(Rect::new(388.0, y, 364.0, 106.0), names[n]);
            self.knob(LFO_FREQ[n], 424.0, y + 62.0, 14.0, "FREQUENCY");
            self.dropdown(LFO_WAVE[n], Rect::new(468.0, y + 26.0, 130.0, 24.0));
            self.switch(LFO_SYNC[n], 614.0, y + 38.0, "SYNC");
            self.switch(LFO_RESET[n], 680.0, y + 38.0, "RESET");
            // The synced rate: n / d of a bar.
            self.dropdown(LFO_SYNCTIME_NUMERATOR[n], Rect::new(468.0, y + 64.0, 62.0, 24.0));
            self.c.text_center(LABEL, Rect::new(530.0, y + 64.0, 14.0, 24.0), plug::DIM, "/");
            self.dropdown(LFO_SYNCTIME_DENOMINATOR[n], Rect::new(544.0, y + 64.0, 62.0, 24.0));
        }
    }

    fn effects(&mut self) {
        let (x0, x1) = (8.0, 388.0);
        // Distortion.
        self.card(Rect::new(x0, 112.0, 364.0, 140.0), "DISTORTION");
        self.switch(DIST_ON, x0 + 20.0, 146.0, "ON");
        self.dropdown(DIST_ALGO, Rect::new(x0 + 90.0, 134.0, 140.0, 24.0));
        self.knobs(&[(DIST_BOOST, "BOOST"), (DIST_DRYWET, "DRY/WET")], x0 + 44.0, 208.0, 70.0, 2, 0.0, 14.0);
        // Delay.
        self.card(Rect::new(x1, 112.0, 364.0, 140.0), "DELAY");
        self.switch(DELAY_ON, x1 + 20.0, 146.0, "ON");
        self.switch(DELAY_PINGPONG, x1 + 80.0, 146.0, "PING PONG");
        let list = [(DELAY_TIME, "TIME"), (DELAY_FEEDBACK, "FEEDBACK"), (DELAY_HP, "HIGH PASS"), (DELAY_DRY, "DRY"), (DELAY_WET, "WET"), (DELAY_DUCKING, "DUCKING")];
        self.knobs(&list, x1 + 36.0, 208.0, 58.0, 6, 0.0, 13.0);
        // Chorus.
        self.card(Rect::new(x0, 258.0, 364.0, 140.0), "CHORUS");
        self.switch(CHORUS_ON, x0 + 20.0, 292.0, "ON");
        self.switch(CHORUS_RESET, x0 + 80.0, 292.0, "RESET");
        let list = [(CHORUS_RATE, "RATE"), (CHORUS_AMOUNT, "AMOUNT"), (CHORUS_FEEDBACK, "FEEDBACK"), (CHORUS_DRYWET, "DRY/WET")];
        self.knobs(&list, x0 + 40.0, 354.0, 72.0, 4, 0.0, 13.0);
        // Flanger.
        self.card(Rect::new(x1, 258.0, 364.0, 140.0), "FLANGER");
        self.switch(FLANGER_ON, x1 + 20.0, 292.0, "ON");
        self.switch(FLANGER_RESET, x1 + 80.0, 292.0, "RESET");
        let list = [(FLANGER_RATE, "RATE"), (FLANGER_AMOUNT, "AMOUNT"), (FLANGER_FEEDBACK, "FEEDBACK"), (FLANGER_DRYWET, "DRY/WET")];
        self.knobs(&list, x1 + 40.0, 354.0, 72.0, 4, 0.0, 13.0);
        // Phaser.
        self.card(Rect::new(x0, 404.0, 364.0, 148.0), "PHASER");
        self.switch(PHASER_ON, x0 + 20.0, 438.0, "ON");
        self.switch(PHASER_RESET, x0 + 80.0, 438.0, "RESET");
        let list = [
            (PHASER_RATE, "RATE"), (PHASER_FREQ, "FREQUENCY"), (PHASER_MOD, "MOD"), (PHASER_FEEDBACK, "FEEDBACK"), (PHASER_DRYWET, "DRY/WET"),
        ];
        self.knobs(&list, x0 + 36.0, 502.0, 70.0, 5, 0.0, 13.0);
        // Reverb.
        self.card(Rect::new(x1, 404.0, 364.0, 148.0), "REVERB");
        self.switch(REVERB_ON, x1 + 20.0, 438.0, "ON");
        let list = [
            (REV_DELAY, "PRE DELAY"), (REV_EQFREQ, "EQ FREQ"), (REV_EQGAIN, "EQ GAIN"), (REV_HF_DAMP, "DAMPING"), (REV_MID_HALL, "SIZE"), (REV_DRYWET, "DRY/WET"),
        ];
        self.knobs(&list, x1 + 36.0, 502.0, 58.0, 6, 0.0, 13.0);
    }

    /// The modulation matrix: nine rows, each a source driving up to two destinations, scaled by another source.
    fn matrix(&mut self) {
        lemon::plug_panel(self.c, Rect::new(8.0, 112.0, 744.0, 440.0));
        let cols = [(16.0, "SOURCE"), (136.0, "DESTINATION 1"), (298.0, "AMOUNT"), (330.0, "DESTINATION 2"), (492.0, "AMOUNT"), (524.0, "SCALE BY"), (652.0, "AMOUNT")];
        for (x, name) in cols {
            self.c.text(CAP, x + 4.0, 118.0, plug::DIM, name);
        }
        for r in 0..9 {
            let y = 134.0 + 46.0 * r as f32;
            self.dropdown(MOD_SOURCE[r], Rect::new(16.0, y + 2.0, 116.0, 24.0));
            self.dropdown(MOD_DEST1[r], Rect::new(136.0, y + 2.0, 150.0, 24.0));
            self.mini(MOD_AMOUNT1[r], 310.0, y + 14.0, 10.0);
            self.dropdown(MOD_DEST2[r], Rect::new(330.0, y + 2.0, 150.0, 24.0));
            self.mini(MOD_AMOUNT2[r], 504.0, y + 14.0, 10.0);
            self.dropdown(MOD_SCALE[r], Rect::new(524.0, y + 2.0, 116.0, 24.0));
            self.mini(MOD_SCALE_AMOUNT[r], 664.0, y + 14.0, 10.0);
        }
    }

    /// The arpeggiator and the voice.
    fn arp(&mut self) {
        self.card(Rect::new(8.0, 112.0, 744.0, 150.0), "ARPEGGIATOR");
        self.switch(ARP_ON, 24.0, 146.0, "ON");
        self.switch(ARP_ONE_SHOT, 80.0, 146.0, "ONE SHOT");
        self.dropdown(ARP_DIRECTION, Rect::new(190.0, 134.0, 150.0, 24.0));
        self.c.text(LABEL, 360.0, 140.0, plug::DIM, "RATE");
        self.dropdown(ARP_SYNCTIME_NUMERATOR, Rect::new(404.0, 134.0, 62.0, 24.0));
        self.c.text_center(LABEL, Rect::new(466.0, 134.0, 14.0, 24.0), plug::DIM, "/");
        self.dropdown(ARP_SYNCTIME_DENOMINATOR, Rect::new(480.0, 134.0, 62.0, 24.0));
        self.knobs(&[(ARP_OCTAVES, "OCTAVES"), (ARP_STEPS, "STEPS"), (ARP_GATE, "GATE")], 50.0, 214.0, 76.0, 3, 0.0, 14.0);
        // The sixteen steps: on, and the transposition.
        self.c.text(CAP, 292.0, 170.0, plug::DIM, "STEPS ON, AND THEIR TRANSPOSITION");
        for s in 0..16 {
            let x = 292.0 + 28.0 * s as f32;
            self.switch(STEP_ON[s], x, 196.0, "");
            self.mini_plain(STEP_TRANSPOSE[s], x, 232.0);
        }
        self.card(Rect::new(8.0, 270.0, 364.0, 282.0), "VOICE");
        self.knobs(&[(UNISON_VOICES, "UNISON"), (UNISON_DETUNE, "DETUNE"), (UNISON_WIDTH, "WIDTH")], 56.0, 336.0, 100.0, 3, 0.0, 16.0);
        self.knobs(&[(PITCHBEND_AMOUNT, "BEND RANGE"), (AMP_PAN, "PAN"), (AMP_VELOCITY, "VELOCITY")], 56.0, 436.0, 100.0, 3, 0.0, 16.0);
        self.card(Rect::new(388.0, 270.0, 364.0, 282.0), "ARP STEPS");
        // Mod values of the steps, in two rows of eight.
        for h in 0..2 {
            for s in 0..8 {
                let i = h * 8 + s;
                let x = 412.0 + 42.0 * s as f32;
                self.c.text_center(CAP, Rect::new(x - 14.0, 296.0 + 120.0 * h as f32, 28.0, 12.0), plug::DIM, &format!("{}", i + 1));
                self.mini(STEP_MOD_1[i], x, 328.0 + 120.0 * h as f32, 9.0);
                self.mini(STEP_MOD_2[i], x, 376.0 + 120.0 * h as f32, 9.0);
            }
        }
        self.c.text(CAP, 400.0, 540.0, plug::DIM, "MOD 1 ABOVE, MOD 2 BELOW (SOURCES ARP MOD 1 AND 2)");
    }

    /// A small value field for a whole-number setting: a tap opens its choices.
    fn mini_plain(&mut self, k: usize, cx: f32, cy: f32) {
        let r = Rect::new(cx - 13.0, cy - 11.0, 26.0, 22.0);
        let st = State { hover: self.hover(r), ..State::NORMAL };
        lemon::field(self.c, r, st);
        let text = self.m.text(SETTINGS[k].id);
        self.c.text_center(CAP, r, brand::MAIN, text.trim_end_matches(" st"));
        self.acts.push((r, Act::Menu(k)));
    }

    fn patches(&mut self, category: usize, list: usize, loaded: Loaded, names: &[String], user: usize) {
        let bank = presets::bank();
        let rows = std::iter::once("ALL".to_string())
            .chain(bank.categories.iter().map(|c| c.to_uppercase()))
            .chain(std::iter::once(format!("FROM FILES ({user})")))
            .collect::<Vec<_>>();
        for (i, name) in rows.iter().enumerate() {
            let r = Rect::new(8.0, 112.0 + 24.5 * i as f32, 170.0, 23.0);
            let st = State { hover: self.hover(r), on: category == i, ..State::NORMAL };
            lemon::field(self.c, r, st);
            self.c.text(LABEL, r.x + 10.0, r.y + 5.0, if category == i { brand::MAIN } else { brand::TEXT }, name);
            self.acts.push((r, Act::Category(i)));
        }
        let shown = Panel::shown(category, user);
        let pages = shown.len().div_ceil(PER_PAGE).max(1);
        let list = list.min(pages - 1);
        if shown.is_empty() {
            self.c.text(ROW, 196.0, 120.0, plug::DIM, "OPEN FILE reads any Odin 2 .odin patch, or drop one here.");
        }
        for (n, &i) in shown.iter().skip(list * PER_PAGE).take(PER_PAGE).enumerate() {
            let (col, row) = (n / 16, n % 16);
            let r = Rect::new(186.0 + 285.0 * col as f32, 112.0 + 24.5 * row as f32, 277.0, 23.0);
            let (here, name, act) = if category == imported() {
                (false, names.get(i).cloned().unwrap_or_default(), Act::User(i))
            } else {
                (loaded == Loaded::Factory(i), bank.patches[i].name.to_string(), Act::Patch(i))
            };
            let st = State { hover: self.hover(r), on: here, ..State::NORMAL };
            lemon::field(self.c, r, st);
            self.c.text(ROW, r.x + 10.0, r.y + 3.0, if here { brand::MAIN } else { brand::TEXT }, &trim(&name, 30));
            self.acts.push((r, act));
        }
        self.button(Rect::new(186.0, 512.0, 40.0, 34.0), "<", Act::List(-1), false);
        let well = Rect::new(230.0, 512.0, 250.0, 34.0);
        lemon::plug_well(self.c, well, 6.0);
        let title = format!("PAGE {}/{}   {} PATCHES", list + 1, pages, shown.len());
        self.c.text_center(Font::new(12).bold(), well, brand::MAIN, &title);
        self.button(Rect::new(484.0, 512.0, 40.0, 34.0), ">", Act::List(1), false);
        self.button(Rect::new(540.0, 512.0, 116.0, 34.0), "OPEN FILE", Act::Open, false);
        self.button(Rect::new(664.0, 512.0, 88.0, 34.0), "INIT", Act::Init, false);
    }
}
