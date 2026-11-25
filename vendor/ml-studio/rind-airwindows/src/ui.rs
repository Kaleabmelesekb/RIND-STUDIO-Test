// SPDX-License-Identifier: GPL-3.0-or-later
//! Airwindows' editor in the ML Studio look: the effect as ten knobs (named and shown as the effect names and shows
//! them), and a page that lists the effects by category. The knobs are the host's parameters (the editor drags them);
//! the pages, the categories and the list are this extension's.
use std::sync::{Arc, Mutex};

use ml_small_fx::editor::Extension;
use ml_small_fx::params::FxParams;
use ml_small_fx::view::{Hit, Model, Target, ViewState};
use nih_plug::prelude::GuiContext;
use os_plugkit::vals::Vals;
use os_theme::design::lemon as brand;
use os_ui::kit::State;
use os_ui::kit::lemon::{self, plug};
use os_ui::{Canvas, Color, Font, Rect};

use crate::list::{CATEGORIES, FX};
use crate::{Engine, FX_SETTING, IDS, PARAMS, fx_at};

const LABEL: Font = Font::new(10).bold();
const CAP: Font = Font::new(10);
const TABS: [&str; 2] = ["SOUND", "EFFECTS"];

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Page(usize),
    Category(usize),
    Fx(usize),
    Step(i32),
}

struct St {
    page: usize,
    category: usize,
    acts: Vec<(Rect, Act)>,
    /// An instance of the effect the knobs show, for the text of their values: the effect's place and the instance.
    view: Option<(usize, Engine)>,
}

pub struct Panel {
    params: Arc<FxParams>,
    st: Mutex<St>,
}

fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}

impl Panel {
    pub fn new(params: Arc<FxParams>) -> Arc<dyn Extension> {
        let category = FX[fx_at(params.v(FX_SETTING))].category;
        let st = St { page: 0, category, acts: Vec::new(), view: None };
        Arc::new(Panel { params, st: Mutex::new(st) })
    }

    /// Effect `index` with its parameters at its defaults: the sound hears it at once and the host is told.
    fn load(&self, index: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let index = index.min(FX.len() - 1);
        let mut vals = Vals::new(&*self.params, self.params.live.clone(), ctx.cloned());
        vals.edit_plain(IDS[FX_SETTING], index as f32);
        for k in 0..PARAMS {
            let d = FX[index].params.get(k).map_or(500, |p| i32::from(p.1));
            vals.edit_plain(IDS[1 + k], d as f32);
        }
        if let Ok(mut st) = self.st.lock() {
            st.category = FX[index].category;
        }
    }

    fn with(&self, f: impl FnOnce(&mut St)) {
        if let Ok(mut st) = self.st.lock() {
            f(&mut st);
        }
    }
}

impl Extension for Panel {
    fn paint(&self, _: &mut Canvas, _: &dyn Model) {}

    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        let act = {
            let Ok(st) = self.st.lock() else { return false };
            let Some(&(_, act)) = st.acts.iter().rev().find(|(r, _)| r.contains(x, y)) else { return false };
            act
        };
        match act {
            Act::Page(p) => self.with(|st| st.page = p),
            Act::Category(c) => self.with(|st| st.category = c),
            Act::Fx(i) => self.load(i, ctx),
            Act::Step(d) => {
                let now = fx_at(self.params.v(FX_SETTING)) as i32;
                self.load((now + d).rem_euclid(FX.len() as i32) as usize, ctx);
            }
        }
        true
    }

    fn render(&self, c: &mut Canvas, m: &dyn Model, state: &ViewState, _hint: &str) -> Option<Vec<Hit>> {
        let current = fx_at(m.plain(IDS[FX_SETTING]).round() as i32);
        let fx = &FX[current];
        let (page, category, texts) = {
            let mut st = self.st.lock().ok()?;
            // The knobs' texts come from the effect itself, on an instance of the editor's own.
            if st.view.as_ref().is_none_or(|(i, _)| *i != current) {
                st.view = Some((current, Engine::new(current, 44100.)));
            }
            let mut texts = Vec::with_capacity(PARAMS);
            if let Some((_, e)) = &st.view {
                for k in 0..fx.params.len() {
                    e.set(k, m.norm(IDS[1 + k]));
                    texts.push(e.display(k));
                }
            }
            (st.page, st.category, texts)
        };
        let mut ui = Ui { c, m, st: state, hits: Vec::new(), acts: Vec::new() };
        ui.c.begin_frame(None);
        ui.c.clear(plug::BG);
        ui.lcd(fx);
        for (i, name) in TABS.iter().enumerate() {
            ui.button(Rect::new(568.0 + 98.0 * i as f32, 8.0, 92.0, 60.0), name, Act::Page(i), page == i);
        }
        match page {
            0 => ui.sound(fx, &texts),
            _ => ui.effects(category, current),
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

    /// The display: the effect, its category and what it is, and the steps to the neighbours.
    fn lcd(&mut self, fx: &crate::list::Fx) {
        let r = Rect::new(8.0, 8.0, 470.0, 60.0);
        lemon::plug_well(self.c, r, 6.0);
        let glow = Color::rgb(0xb7e36a);
        let dim = Color::rgb(0x7f9a54);
        self.c.text(Font::new(20).bold(), r.x + 14.0, r.y + 6.0, glow, fx.name);
        let sub = format!("{}  {}", CATEGORIES[fx.category].to_uppercase(), fx.what);
        self.c.text(CAP, r.x + 16.0, r.y + 38.0, dim, &trim(&sub, 78));
        self.button(Rect::new(484.0, 8.0, 36.0, 60.0), "<", Act::Step(-1), false);
        self.button(Rect::new(524.0, 8.0, 36.0, 60.0), ">", Act::Step(1), false);
    }

    /// A dial for parameter `k` with its name over it and the effect's text for its value under it.
    fn knob(&mut self, k: usize, cx: f32, cy: f32, r: f32, caption: &str, text: &str) {
        let id = IDS[1 + k];
        let target = Target::Knob(id.to_string());
        let hr = Rect::new(cx - r - 4.0, cy - r - 4.0, 2.0 * r + 8.0, 2.0 * r + 8.0);
        let st = self.state(hr, &target);
        lemon::dial(self.c, cx, cy, r, 3.2, self.m.norm(id), false, brand::MAIN, st);
        self.c.text_center(CAP, Rect::new(cx - 60.0, cy - r - 19.0, 120.0, 13.0), plug::DIM, &trim(&caption.to_uppercase(), 16));
        self.c.text_center(LABEL, Rect::new(cx - 60.0, cy + r + 5.0, 120.0, 13.0), brand::MAIN, &trim(text, 18));
        self.hits.push(Hit { r: hr, t: target });
    }

    fn sound(&mut self, fx: &crate::list::Fx, texts: &[String]) {
        self.card(Rect::new(8.0, 76.0, 744.0, 386.0), "PARAMETERS");
        if fx.params.is_empty() {
            self.c.text_center(
                Font::new(14).bold(),
                Rect::new(8.0, 76.0, 744.0, 386.0),
                plug::DIM,
                "THIS EFFECT HAS NO CONTROLS",
            );
            return;
        }
        for (k, (name, _)) in fx.params.iter().enumerate() {
            let (row, col) = (k / 5, k % 5);
            let cx = 83.0 + 149.0 * col as f32;
            let cy = 170.0 + 150.0 * row as f32;
            self.knob(k, cx, cy, 30.0, name, texts.get(k).map_or("", |s| s.as_str()));
        }
    }

    fn effects(&mut self, category: usize, current: usize) {
        for (i, name) in CATEGORIES.iter().enumerate() {
            let (row, col) = (i / 7, i % 7);
            let r = Rect::new(8.0 + 107.0 * col as f32, 76.0 + 30.0 * row as f32, 101.0, 26.0);
            self.button(r, &trim(&name.to_uppercase(), 11), Act::Category(i), category == i);
        }
        self.card(Rect::new(8.0, 172.0, 744.0, 290.0), "EFFECTS");
        let mut n = 0;
        for (i, fx) in FX.iter().enumerate().filter(|(_, f)| f.category == category) {
            let (row, col) = (n / 4, n % 4);
            let r = Rect::new(18.0 + 182.0 * col as f32, 196.0 + 34.0 * row as f32, 176.0, 30.0);
            self.button(r, &trim(fx.name, 18), Act::Fx(i), i == current);
            n += 1;
        }
    }
}
