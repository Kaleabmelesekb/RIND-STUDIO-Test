//! The Browser and the Device Rack: the left and bottom docks of the Ableton / Bitwig style layout, around the
//! arrangement.
//!
//! - The Browser (`app.regions.browser`) is a collapsible column down the left of the content area with four tabs:
//!   Instruments and Effects (the Modules browser's list: a tap adds the device to the selected track's rack), Samples
//!   (the sample library) and Projects (the song library: a tap opens the project). Collapsed it is a thin rail.
//! - The Device Rack (`app.regions.device_rack`) lies under the arrangement: a header with the selected track's name
//!   and, below it, the track's own device cards side by side ([`rack::show_strip`]), scrolling along x. Collapsed it
//!   is its header alone; `Expand` opens the side rack, the full rack of the track.
//!
//! Every panel is flat: square corners and 1 px lines in the text colour at a low alpha, with no shadow or gradient.
//! The state ([`Docks`]) outlives a view change, so the tab, the folders entered and the open or collapsed state stay.

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use super::{AppCtx, Nav, Overlay, modules_browser as picks, preset_picker, rack};
use crate::metrics::{FontSize, Metrics};
use crate::project_files::Replace;
use crate::song::{LaneKind, ModuleInstance};
use crate::ui::main_menu::library::SongEntry;
use crate::ui::modules_browser::{
    BrowserAction, Entry, InsertAt, ModuleOrdering, Row, Takes, rows,
};
use crate::ui::widgets;

/// The Browser's width when open in a landscape window, in U; a portrait window's is [`PORTRAIT_W`].
const BROWSER_W: f32 = 9.0;
const PORTRAIT_W: f32 = 10.0;
/// The collapsed Browser's rail, in U.
const RAIL_W: f32 = 1.25;
/// The headers' height, the Browser's tab row and a list row, in U: touch sized.
const HEADER_H: f32 = 1.5;
const TAB_H: f32 = 1.5;
const ROW_H: f32 = 1.5;
/// The Device Rack's body when open: a fixed height in U, however the window is sized (only a window too short for it
/// gives it less, [`MAX_SHARE`]).
const RACK_BODY_H: f32 = 7.5;
/// The most of the content area either dock takes.
const MAX_SHARE: f32 = 0.7;
/// A row's left padding, in U.
const PAD: f32 = 0.5;

/// A Browser tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Instruments,
    Effects,
    Samples,
    Projects,
}

impl Tab {
    const ALL: [Self; 4] = [
        Self::Instruments,
        Self::Effects,
        Self::Samples,
        Self::Projects,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Instruments => "Instr",
            Self::Effects => "FX",
            Self::Samples => "Samples",
            Self::Projects => "Projects",
        }
    }

    /// The folder stack's slot: Samples and Projects have one each.
    fn slot(self) -> Option<usize> {
        match self {
            Self::Samples => Some(0),
            Self::Projects => Some(1),
            _ => None,
        }
    }
}

/// What the docks keep between frames.
pub struct Docks {
    /// The docks are laid out and drawn. Off in the unit tests, whose screens place everything by position.
    pub enabled: bool,
    pub browser_open: bool,
    pub rack_open: bool,
    pub tab: Tab,
    /// The first frame decides the portrait layout: the Browser starts collapsed there.
    placed: bool,
    /// The folders entered in Samples and Projects, outermost first.
    folders: [Vec<PathBuf>; 2],
    /// The list of the Instruments and Effects tabs, kept while the tab and the track's kind stay.
    modules: Option<((Tab, Takes), Vec<Row>)>,
    /// The folder listing of the Samples and Projects tabs, kept while the tab, the folder and the library stay.
    files: Option<(FilesKey, Vec<SongEntry>)>,
    /// A device chip's tap: the card to scroll into view in the strip.
    pub reveal: Option<usize>,
}

type FilesKey = (Tab, Option<PathBuf>, u64);

impl Default for Docks {
    fn default() -> Self {
        Self {
            enabled: !cfg!(test),
            browser_open: true,
            rack_open: true,
            tab: Tab::default(),
            placed: false,
            folders: [Vec::new(), Vec::new()],
            modules: None,
            files: None,
            reveal: None,
        }
    }
}

/// What a tap in the Browser's list asks for.
enum Click {
    Pick(BrowserAction),
    Enter(PathBuf),
    Up,
    Open(PathBuf),
    Sample,
}

/// One row of the Browser's list.
enum Item<'a> {
    Section(&'static str),
    Module(Entry),
    Up,
    Folder(&'a SongEntry),
    File(&'a SongEntry),
    Note(&'static str),
}

impl Docks {
    /// The width of the Browser and the height of the Device Rack for the content `area`, in points. The rack is out
    /// (height 0) while the side rack is `racked`: it shows the same devices.
    pub fn insets(&mut self, area: Rect, portrait: bool, racked: bool, m: &Metrics) -> (f32, f32) {
        if !self.placed {
            self.placed = true;
            self.browser_open &= !portrait;
        }
        let wide = if portrait { PORTRAIT_W } else { BROWSER_W };
        let browser = if self.browser_open {
            m.u(wide).min(area.width() * MAX_SHARE)
        } else {
            m.u(RAIL_W)
        };
        let header = m.u(HEADER_H).min(area.height() * 0.5);
        let rack = if racked {
            0.0
        } else if self.rack_open {
            (m.u(HEADER_H + RACK_BODY_H))
                .min(area.height() * MAX_SHARE)
                .max(header)
        } else {
            header
        };
        (browser, rack)
    }

    /// Draws the Browser in `app.regions.browser` and does what a tap in it asks. `picker` is the preset picker's state
    /// (a new DW Sampler loads Close Grand).
    pub fn browser(
        &mut self,
        ui: &mut Ui,
        app: &mut AppCtx,
        picker: &mut preset_picker::State,
    ) -> Option<Nav> {
        let rect = app.regions.browser;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return None;
        }
        let m = app.cx.m;
        let look = Look::of(&app.cx);
        let painter = ui.painter().clone();
        let covered = app.covered;
        painter.rect_filled(rect, 0.0, look.bar);
        // Its right edge: a 1 px line.
        let edge = [rect.right_top(), rect.right_bottom()];
        painter.line_segment(
            edge.map(|p| p - vec2(look.px / 2.0, 0.0)),
            Stroke::new(look.px, look.line),
        );
        let sense = if covered {
            Sense::hover()
        } else {
            Sense::click()
        };

        if !self.browser_open {
            // The rail: a chevron and BROWSER down it, the whole rail opens it.
            let open = ui.interact(rect, egui::Id::new("dock browser rail"), sense);
            let tone = if widgets::hovered(ui, &open) {
                look.text
            } else {
                look.dim
            };
            chevron(
                &painter,
                pos2(rect.center().x, rect.top() + m.u(0.9)),
                m.u(0.3),
                Dir::Right,
                tone,
                look.px,
            );
            let font = m.font_id(FontSize::Pt8);
            for (i, c) in "BROWSER".chars().enumerate() {
                let at = pos2(rect.center().x, rect.top() + m.u(2.2 + 0.7 * i as f32));
                painter.text(at, Align2::CENTER_CENTER, c, font.clone(), tone);
            }
            if open.clicked() {
                self.browser_open = true;
            }
            return None;
        }

        // The header: the title and the collapse chevron.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), m.u(HEADER_H)));
        painter.text(
            pos2(rect.left() + m.u(PAD), header.center().y),
            Align2::LEFT_CENTER,
            "BROWSER",
            m.font_id(FontSize::Pt10),
            look.text,
        );
        let hide = Rect::from_min_size(
            pos2(header.right() - m.u(HEADER_H), header.top()),
            vec2(m.u(HEADER_H), header.height()),
        );
        let hide_r = ui.interact(hide, egui::Id::new("dock browser hide"), sense);
        let tone = if widgets::hovered(ui, &hide_r) {
            look.text
        } else {
            look.dim
        };
        chevron(&painter, hide.center(), m.u(0.3), Dir::Left, tone, look.px);
        if hide_r.clicked() {
            self.browser_open = false;
        }
        rule(&painter, header.left_bottom(), header.right_bottom(), &look);

        // The tabs, a flat row: the open one on the panel's colour with an accent line under it.
        let tabs = Rect::from_min_size(header.left_bottom(), vec2(rect.width(), m.u(TAB_H)));
        let cell = tabs.width() / Tab::ALL.len() as f32;
        for (i, tab) in Tab::ALL.into_iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(tabs.left() + cell * i as f32, tabs.top()),
                vec2(cell, tabs.height()),
            );
            let response = ui.interact(r, egui::Id::new(("dock browser tab", i)), sense);
            let on = self.tab == tab;
            if on {
                painter.rect_filled(r, 0.0, look.bg);
                let line = Rect::from_min_size(
                    pos2(r.left(), r.bottom() - 2.0 * look.px),
                    vec2(r.width(), 2.0 * look.px),
                );
                painter.rect_filled(line, 0.0, look.accent);
            }
            let tone = match (on, widgets::hovered(ui, &response)) {
                (true, _) | (_, true) => look.text,
                _ => look.dim,
            };
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                tab.label(),
                m.font_id(FontSize::Pt8),
                tone,
            );
            if response.clicked() {
                self.tab = tab;
            }
        }
        rule(&painter, tabs.left_bottom(), tabs.right_bottom(), &look);

        let list = Rect::from_min_max(tabs.left_bottom(), rect.right_bottom() - vec2(look.px, 0.0));
        self.list(ui, app, picker, list, &look)
    }

    /// The Browser's list in `list`.
    fn list(
        &mut self,
        ui: &mut Ui,
        app: &mut AppCtx,
        picker: &mut preset_picker::State,
        list: Rect,
        look: &Look,
    ) -> Option<Nav> {
        let m = app.cx.m;
        let selected = *app.selected;
        let tab = self.tab;
        // What the tab lists, read again when the tab, the folder or the library changes.
        match tab.slot() {
            None => {
                let takes = app.song.channels.get(selected).map(crate::ui::rack::takes);
                let key = (tab, takes.unwrap_or(Takes::Effects));
                if self.modules.as_ref().is_none_or(|(k, _)| *k != key) {
                    let all = rows("", ModuleOrdering::Category, key.1);
                    self.modules = Some((key, of_tab(tab, &all)));
                }
            }
            Some(slot) => {
                let folder = self.folders[slot].last().cloned();
                let key: FilesKey = (tab, folder, app.files.library_changes());
                if self.files.as_ref().is_none_or(|(k, _)| *k != key) {
                    let read = match tab {
                        Tab::Samples => app.files.sample_library().list(key.1.as_deref()),
                        _ => app.files.library.list(key.1.as_deref()),
                    };
                    let mut entries = read.unwrap_or_default();
                    entries.sort_by(|a, b| {
                        (b.is_folder.cmp(&a.is_folder))
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                    self.files = Some((key, entries));
                }
            }
        }
        let mut items: Vec<Item> = Vec::new();
        match tab.slot() {
            None => {
                let rows = self.modules.as_ref().map_or(&[][..], |(_, r)| &r[..]);
                for row in rows {
                    items.push(match *row {
                        Row::Section(label) => Item::Section(label),
                        Row::Item(entry) => Item::Module(entry),
                    });
                }
                if items.is_empty() {
                    items.push(Item::Note(match tab {
                        Tab::Instruments => "This track takes no instruments",
                        _ => "No effects",
                    }));
                }
            }
            Some(slot) => {
                if !self.folders[slot].is_empty() {
                    items.push(Item::Up);
                }
                let entries = self.files.as_ref().map_or(&[][..], |(_, e)| &e[..]);
                for e in entries {
                    items.push(if e.is_folder {
                        Item::Folder(e)
                    } else {
                        Item::File(e)
                    });
                }
                if items.is_empty() {
                    items.push(Item::Note("Nothing here"));
                }
            }
        }

        let covered = app.covered;
        let (row_h, pad) = (m.u(ROW_H), m.u(PAD));
        let font = m.font_id(FontSize::Pt10);
        let small = m.font_id(FontSize::Pt8);
        let mut click = None;
        let mut child = ui.new_child(UiBuilder::new().max_rect(list).id(("dock list", tab as u8)));
        child.set_clip_rect(list.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(("dock list scroll", tab as u8))
            .auto_shrink([false, false])
            .show_rows(&mut child, row_h, items.len(), |ui, range| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for item in &items[range] {
                    let active = !covered && !matches!(item, Item::Section(_) | Item::Note(_));
                    let sense = if active {
                        Sense::click()
                    } else {
                        Sense::hover()
                    };
                    let (r, response) = ui.allocate_exact_size(vec2(list.width(), row_h), sense);
                    let painter = ui.painter();
                    let lit = active && widgets::hovered(ui, &response);
                    if let Item::Section(_) = item {
                        painter.rect_filled(r, 0.0, look.bar);
                    } else if lit {
                        painter.rect_filled(r, 0.0, look.hover);
                    }
                    let at = pos2(r.left() + pad, r.center().y);
                    match item {
                        Item::Section(label) => {
                            painter.text(at, Align2::LEFT_CENTER, label, small.clone(), look.dim);
                        }
                        Item::Note(text) => {
                            painter.text(at, Align2::LEFT_CENTER, text, small.clone(), look.dim);
                        }
                        Item::Up => {
                            chevron(
                                painter,
                                pos2(r.left() + pad + m.u(0.2), r.center().y),
                                m.u(0.22),
                                Dir::Left,
                                look.text,
                                look.px,
                            );
                            painter.text(
                                pos2(at.x + m.u(0.8), at.y),
                                Align2::LEFT_CENTER,
                                "Back",
                                font.clone(),
                                look.text,
                            );
                        }
                        Item::Folder(e) => {
                            chevron(
                                painter,
                                pos2(r.left() + pad + m.u(0.2), r.center().y),
                                m.u(0.22),
                                Dir::Right,
                                look.dim,
                                look.px,
                            );
                            let name = ellipsis(
                                painter,
                                &e.name,
                                &font,
                                look.text,
                                r.width() - pad - m.u(1.2),
                            );
                            painter.text(
                                pos2(at.x + m.u(0.8), at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.text,
                            );
                        }
                        Item::File(e) => {
                            let name =
                                ellipsis(painter, &e.name, &font, look.text, r.width() - 2.0 * pad);
                            painter.text(at, Align2::LEFT_CENTER, name, font.clone(), look.text);
                        }
                        Item::Module(entry) => {
                            let name = ellipsis(
                                painter,
                                entry.name(),
                                &font,
                                look.text,
                                r.width() - pad - m.u(1.2),
                            );
                            painter.text(at, Align2::LEFT_CENTER, name, font.clone(), look.text);
                            // A + at the right: a tap adds it.
                            let plus = pos2(r.right() - m.u(0.7), r.center().y);
                            let arm = m.u(0.2);
                            let stroke = Stroke::new(look.px, look.dim);
                            painter.line_segment(
                                [plus - vec2(arm, 0.0), plus + vec2(arm, 0.0)],
                                stroke,
                            );
                            painter.line_segment(
                                [plus - vec2(0.0, arm), plus + vec2(0.0, arm)],
                                stroke,
                            );
                        }
                    }
                    if !matches!(item, Item::Section(_)) {
                        rule(painter, r.left_bottom(), r.right_bottom(), look);
                    }
                    if response.clicked() {
                        click = match item {
                            Item::Module(entry) => Some(Click::Pick(entry.action())),
                            Item::Up => Some(Click::Up),
                            Item::Folder(e) => Some(Click::Enter(e.path.clone())),
                            Item::File(e) if tab == Tab::Projects => {
                                Some(Click::Open(e.path.clone()))
                            }
                            Item::File(_) => Some(Click::Sample),
                            _ => None,
                        };
                    }
                }
            });

        match click? {
            Click::Pick(action) => {
                app.song.channels.get(selected)?;
                picks::add_pick(picker, app, selected, action);
                // The new card is the last one: the strip shows it.
                if let Some(ch) = app.song.channels.get(selected) {
                    self.reveal = ch.rack.modules.len().checked_sub(1);
                }
                self.rack_open = true;
                None
            }
            Click::Enter(path) => {
                if let Some(slot) = tab.slot() {
                    self.folders[slot].push(path);
                }
                None
            }
            Click::Up => {
                if let Some(slot) = tab.slot() {
                    self.folders[slot].pop();
                }
                None
            }
            Click::Open(path) => {
                super::main_menu::replace(app, Replace::Open(path));
                None
            }
            Click::Sample => {
                // A sample goes onto the selected track's first audio lane, through the sample picker's own audition
                // and fit: it opens at bar 1 (a tap on an empty cell of a lane opens it at that cell).
                let lane = (app.song.channels.get(selected))
                    .and_then(|c| c.lanes.iter().position(|l| l.kind == LaneKind::Audio));
                match lane {
                    Some(lane) => Some(Nav::Open(Overlay::TrackSamplePicker {
                        channel: selected,
                        lane,
                        tick: 0,
                    })),
                    None => {
                        app.files.flash("Select an audio track to add a sample");
                        None
                    }
                }
            }
        }
    }

    /// Draws the Device Rack in `app.regions.device_rack`: its header and, while open, the strip of the selected track's
    /// cards. `Expand` opens the side rack and `+` the Modules browser, both returned as the navigation they are.
    pub fn device_rack(
        &mut self,
        ui: &mut Ui,
        app: &mut AppCtx,
        state: &mut rack::State,
        clipboard: &mut Vec<ModuleInstance>,
        held: Option<(usize, u32)>,
        draw_strip: bool,
    ) -> Option<Nav> {
        let rect = app.regions.device_rack;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return None;
        }
        let m = app.cx.m;
        let look = Look::of(&app.cx);
        let painter = ui.painter().clone();
        let selected = *app.selected;
        let covered = app.covered;
        let sense = if covered {
            Sense::hover()
        } else {
            Sense::click()
        };
        let name = app.song.channels.get(selected).map(|c| c.name.clone());

        let header = Rect::from_min_size(
            rect.min,
            vec2(rect.width(), m.u(HEADER_H).min(rect.height())),
        );
        painter.rect_filled(header, 0.0, look.bar);
        rule(&painter, rect.left_top(), rect.right_top(), &look);
        // The collapse chevron, then the title.
        let toggle = Rect::from_min_size(header.min, vec2(m.u(HEADER_H), header.height()));
        let toggled = ui.interact(toggle, egui::Id::new("dock rack toggle"), sense);
        let tone = if widgets::hovered(ui, &toggled) {
            look.text
        } else {
            look.dim
        };
        let dir = if self.rack_open {
            Dir::Down
        } else {
            Dir::Right
        };
        chevron(&painter, toggle.center(), m.u(0.3), dir, tone, look.px);
        if toggled.clicked() {
            self.rack_open ^= true;
        }
        let title = match &name {
            Some(name) => format!("DEVICES  {name}"),
            None => "DEVICES".to_owned(),
        };
        painter.text(
            pos2(toggle.right(), header.center().y),
            Align2::LEFT_CENTER,
            title,
            m.font_id(FontSize::Pt10),
            look.text,
        );
        // The buttons at the right: `+` and `Expand`, flat boxes with a 1 px outline.
        let mut nav = None;
        let font = m.font_id(FontSize::Pt8);
        let expand = Rect::from_min_size(
            pos2(header.right() - m.u(5.0), header.top() + m.u(0.2)),
            vec2(m.u(4.5), header.height() - m.u(0.4)),
        );
        let add = Rect::from_min_size(
            pos2(expand.left() - m.u(2.0), expand.top()),
            vec2(m.u(1.75), expand.height()),
        );
        for (i, (r, label)) in [(add, "+"), (expand, "Expand")].into_iter().enumerate() {
            if r.left() < toggle.right() + m.u(6.0) {
                continue;
            }
            let response = ui.interact(r, egui::Id::new(("dock rack button", i)), sense);
            let fill = if widgets::hovered(ui, &response) {
                look.hover
            } else {
                look.bg
            };
            painter.rect_filled(r, 0.0, fill);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(look.px, look.line),
                egui::StrokeKind::Inside,
            );
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                look.text,
            );
            if response.clicked() && name.is_some() {
                nav = Some(match i {
                    0 => Nav::Open(Overlay::ModulesBrowser {
                        channel: selected,
                        at: InsertAt::End,
                    }),
                    _ => Nav::Open(Overlay::Rack(selected)),
                });
            }
        }
        rule(&painter, header.left_bottom(), header.right_bottom(), &look);

        let body = rect.with_min_y(header.bottom());
        if !draw_strip || !self.rack_open || body.height() < 1.0 || name.is_none() {
            return nav;
        }
        let reveal = self.reveal.take();
        let strip = rack::show_strip(state, ui, app, body, selected, clipboard, held, reveal);
        nav.or(strip)
    }
}

/// The colours and the line width the docks draw with, from the theme: flat fills, 1 px lines.
struct Look {
    bg: Color32,
    bar: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    line: Color32,
    hover: Color32,
    /// One device pixel in points.
    px: f32,
}

impl Look {
    fn of(cx: &crate::ui::UiCtx) -> Self {
        let c = &cx.theme.colors;
        let text = c.text.to_color32();
        Self {
            bg: c.background.to_color32(),
            bar: c.control_fill.to_color32(),
            text,
            dim: text.gamma_multiply(0.6),
            accent: c.accent.to_color32(),
            line: text.gamma_multiply(0.3),
            hover: crate::ui::buttons::hover_lit(c.control_fill.to_color32()),
            px: 1.0 / cx.m.ppp(),
        }
    }
}

/// A 1 px line from `a` to `b`.
fn rule(painter: &egui::Painter, a: Pos2, b: Pos2, look: &Look) {
    painter.line_segment([a, b], Stroke::new(look.px, look.line));
}

#[derive(Clone, Copy)]
enum Dir {
    Left,
    Right,
    Down,
}

/// A chevron `size` big at `center`, two 1 px strokes (no glyph: every font draws it the same).
fn chevron(painter: &egui::Painter, center: Pos2, size: f32, dir: Dir, color: Color32, px: f32) {
    let (a, b, tip) = match dir {
        Dir::Left => (vec2(size, -size), vec2(size, size), vec2(-size, 0.0)),
        Dir::Right => (vec2(-size, -size), vec2(-size, size), vec2(size, 0.0)),
        Dir::Down => (vec2(-size, -size), vec2(size, -size), vec2(0.0, size)),
    };
    let stroke = Stroke::new(2.0 * px, color);
    painter.line_segment([center + a, center + tip], stroke);
    painter.line_segment([center + b, center + tip], stroke);
}

/// `text` cut to `room` points, ending in an ellipsis when it does not fit.
fn ellipsis(
    painter: &egui::Painter,
    text: &str,
    font: &egui::FontId,
    color: Color32,
    room: f32,
) -> String {
    let fits = |t: &str| {
        painter
            .layout_no_wrap(t.to_owned(), font.clone(), color)
            .size()
            .x
            <= room
    };
    if fits(text) {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while cut.chars().count() > 1 && !fits(&format!("{cut}…")) {
        cut.pop();
    }
    cut.push('…');
    cut
}

/// The Modules browser's rows for `tab`: its instruments (`Instruments`) or its effects and note effects (`Effects`),
/// each with its section header while one of its entries is listed.
fn of_tab(tab: Tab, all: &[Row]) -> Vec<Row> {
    let instrument = |e: Entry| match e {
        Entry::Synth(_) => true,
        Entry::Plugin(p) => p.instrument,
        _ => false,
    };
    let mut out = Vec::new();
    let mut head = None;
    for &row in all {
        match row {
            Row::Section(label) => head = Some(label),
            Row::Item(Entry::Paste | Entry::RackPreset | Entry::Instrument) => {}
            Row::Item(entry) => {
                if instrument(entry) == (tab == Tab::Instruments) {
                    if let Some(label) = head.take() {
                        out.push(Row::Section(label));
                    }
                    out.push(Row::Item(entry));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Metrics;

    fn area() -> Rect {
        Rect::from_min_size(pos2(0.0, 56.0), vec2(1170.0, 300.0))
    }

    #[test]
    fn the_docks_start_open_in_landscape_and_the_browser_collapsed_in_portrait() {
        let m = Metrics::from_pixels_per_point(1.0, 100);
        let mut landscape = Docks::default();
        let (browser, rack) = landscape.insets(area(), false, false, &m);
        assert!(browser > m.u(RAIL_W) && browser <= m.u(BROWSER_W));
        assert!(rack > m.u(HEADER_H) && rack <= 300.0 * MAX_SHARE + 0.01);
        let mut portrait = Docks::default();
        let (browser, _) = portrait.insets(area(), true, false, &m);
        assert_eq!(browser, m.u(RAIL_W));
        assert!(!portrait.browser_open);
    }

    #[test]
    fn collapsing_leaves_a_rail_and_a_header_and_the_side_rack_takes_the_rack_out() {
        let m = Metrics::from_pixels_per_point(1.0, 100);
        let mut docks = Docks {
            browser_open: false,
            rack_open: false,
            ..Docks::default()
        };
        let header = m.u(HEADER_H).min(area().height() * 0.5);
        assert_eq!(
            docks.insets(area(), false, false, &m),
            (m.u(RAIL_W), header)
        );
        docks.rack_open = true;
        assert_eq!(docks.insets(area(), false, true, &m).1, 0.0);
    }

    #[test]
    fn each_tab_lists_its_own_kind() {
        let all = rows("", ModuleOrdering::Category, Takes::Instruments);
        let synths = |tab| {
            of_tab(tab, &all)
                .iter()
                .filter(|r| matches!(r, Row::Item(Entry::Synth(_))))
                .count()
        };
        assert!(synths(Tab::Instruments) > 0);
        assert_eq!(synths(Tab::Effects), 0);
        let effects = of_tab(Tab::Effects, &all);
        assert!(
            effects
                .iter()
                .any(|r| matches!(r, Row::Item(Entry::Module(_))))
        );
        // No action row (`Paste`, `Rack Preset`, `Instrument`) is listed.
        for tab in [Tab::Instruments, Tab::Effects] {
            let listed = of_tab(tab, &all);
            assert!(!listed.iter().any(|r| matches!(
                r,
                Row::Item(Entry::Paste | Entry::RackPreset | Entry::Instrument)
            )));
        }
    }

    #[test]
    fn the_view_gives_way_to_the_docks_and_is_whole_again_without_them() {
        let m = Metrics::from_pixels_per_point(1.0, 100);
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1170.0, 800.0));
        let whole = crate::ui::main_screen::Regions::new(screen, &m);
        let docked = whole.with_docks(m.u(9.0), m.u(6.0), &m);
        assert_eq!(docked.browser.left(), whole.area.left());
        assert_eq!(docked.browser.height(), whole.area.height());
        assert_eq!(docked.area.left(), docked.browser.right());
        assert_eq!(docked.area.bottom(), docked.device_rack.top());
        assert_eq!(docked.device_rack.bottom(), whole.area.bottom());
        assert_eq!(docked.editor, docked.area);
        let bare = whole.with_docks(0.0, 0.0, &m);
        assert_eq!((bare.area, bare.editor), (whole.area, whole.area));
    }
}
