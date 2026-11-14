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

use eframe::egui::{
    self, Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, UiBuilder, pos2, vec2,
};

use super::{AppCtx, Nav, Overlay, modules_browser as picks, preset_picker, rack};
use crate::device_storage::{self, on_storage, pins, save_pins};
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
/// How far right of the list, in U, a press on a row must be dragged for it to leave the list.
const DRAG_OUT: f32 = 0.6;
/// The headers' height, the Browser's tab row and a list row, in U: touch sized.
const HEADER_H: f32 = 1.5;
const TAB_H: f32 = 1.5;
const ROW_H: f32 = 1.5;
/// The Device Rack's body when open: a fixed height in U, however the window is sized (only a window too short for it
/// gives it less, [`MAX_SHARE`]).
const RACK_BODY_H: f32 = 7.5;
/// How far a hand may stretch the Browser (its width) and the Device Rack (its body's height), in U.
const BROWSER_RANGE: (f32, f32) = (5.0, 26.0);
const RACK_RANGE: (f32, f32) = (4.0, 30.0);
/// The grab strip on a dock's edge, in U.
const GRIP: f32 = 0.6;
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
    /// The Browser's width in U once a hand has stretched it (0: the default for the window), and the Device Rack's
    /// body height in U.
    browser_w: f32,
    rack_body: f32,
    /// The folders entered in Samples and Projects, outermost first.
    folders: [Vec<PathBuf>; 2],
    /// The list of the Instruments and Effects tabs, kept while the tab and the track's kind stay.
    modules: Option<((Tab, Takes), Vec<Row>)>,
    /// The folder listing of the Samples and Projects tabs, kept while the tab, the folder and the library stay.
    files: Option<(FilesKey, Vec<SongEntry>)>,
    /// A device chip's tap: the card to scroll into view in the strip.
    pub reveal: Option<usize>,
    /// A browser row being dragged out to a track or the Device Rack: what it adds and its name.
    drag: Option<(BrowserAction, String)>,
    /// The folders of the internal storage the user pinned as places (the user folder's `browser-places.txt`), read on
    /// first use.
    pins: Vec<PathBuf>,
    pins_read: bool,
    /// Whether the app may read the internal storage, and when that was last asked (s): about once a second.
    storage: (bool, f64),
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
            browser_w: 0.0,
            rack_body: RACK_BODY_H,
            folders: [Vec::new(), Vec::new()],
            modules: None,
            files: None,
            reveal: None,
            drag: None,
            pins: Vec::new(),
            pins_read: false,
            storage: (false, f64::NEG_INFINITY),
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
    AddFolder,
    /// Open the system page that gives the app the internal storage.
    Grant,
}

/// One row of the Browser's list.
enum Item<'a> {
    Section(&'static str),
    Module(Entry),
    /// The folder being listed: a tap goes up one level.
    Up(String),
    /// A place at the top: its name, folder, and whether it is a pinned one.
    Place(String, PathBuf, bool),
    /// The internal storage is not readable yet: a tap asks for it.
    Grant,
    Folder(&'a SongEntry),
    File(&'a SongEntry),
    Note(&'static str),
    /// The Samples tab's first row: pick a folder of the device's storage to bring into the library.
    AddFolder,
}

impl Docks {
    /// The width of the Browser and the height of the Device Rack for the content `area`, in points. The rack is out
    /// (height 0) while the side rack is `racked`: it shows the same devices.
    pub fn insets(&mut self, area: Rect, portrait: bool, racked: bool, m: &Metrics) -> (f32, f32) {
        if !self.placed {
            self.placed = true;
            self.browser_open &= !portrait;
        }
        let wide = match self.browser_w {
            w if w > 0.0 => w,
            _ if portrait => PORTRAIT_W,
            _ => BROWSER_W,
        };
        let browser = if self.browser_open {
            m.u(wide).min(area.width() * MAX_SHARE)
        } else {
            m.u(RAIL_W)
        };
        let header = m.u(HEADER_H).min(area.height() * 0.5);
        let rack = if racked {
            0.0
        } else if self.rack_open {
            (m.u(HEADER_H + self.rack_body))
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
        let nav = self.list(ui, app, picker, list, &look);
        // The grab strip on its right edge: drag to stretch the Browser, double tap to give it back its width.
        let grip = Rect::from_x_y_ranges(rect.right() - m.u(GRIP)..=rect.right(), rect.y_range());
        let drag = ui.interact(
            grip,
            egui::Id::new("dock browser grip"),
            grab_sense(covered),
        );
        let width = rect.width() / m.u(1.0);
        if drag.dragged() {
            self.browser_w =
                (width + drag.drag_delta().x / m.u(1.0)).clamp(BROWSER_RANGE.0, BROWSER_RANGE.1);
        } else if drag.double_clicked() {
            self.browser_w = 0.0;
        }
        paint_grip(&painter, grip, false, &drag, &look, ui);
        nav
    }

    /// A browser row in flight: a chip at the pointer, the track row or the Device Rack under it lit; letting go over
    /// one returns the channel to add to and what to add. `rows` are the playlist's track rows as last drawn.
    pub fn drag(
        &mut self,
        ui: &Ui,
        app: &AppCtx,
        rows: &[(Rect, usize)],
    ) -> Option<(usize, BrowserAction)> {
        let (action, label) = self.drag.clone()?;
        let pos = ui.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()));
        let target = pos.and_then(|p| {
            let row = rows
                .iter()
                .find(|(r, _)| r.contains(p))
                .map(|&(r, c)| (c, r));
            let rack = [app.regions.device_rack, app.regions.rack]
                .into_iter()
                .find(|r| r.width() > 0.0 && r.contains(p));
            row.or_else(|| rack.map(|r| (*app.selected, r)))
        });
        if ui.input(|i| i.pointer.any_down()) {
            let m = app.cx.m;
            let look = Look::of(&app.cx);
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("dock drag"));
            let painter = ui.ctx().layer_painter(layer);
            if let Some((_, area)) = target {
                painter.rect_filled(area, 0.0, look.accent.gamma_multiply(0.16));
                painter.rect_stroke(
                    area.shrink(look.px),
                    0.0,
                    Stroke::new(2.0 * look.px, look.accent),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(p) = pos {
                let galley = painter.layout_no_wrap(
                    format!("+ {label}"),
                    m.font_id(FontSize::Pt10),
                    look.bg,
                );
                let chip = Rect::from_min_size(
                    p + vec2(m.u(0.6), m.u(0.3)),
                    galley.size() + vec2(m.u(0.8), m.u(0.5)),
                );
                painter.rect_filled(chip, m.u(0.2), look.accent);
                painter.galley(chip.min + vec2(m.u(0.4), m.u(0.25)), galley, look.bg);
            }
            ui.ctx().request_repaint();
            return None;
        }
        self.drag = None;
        target.map(|(channel, _)| (channel, action))
    }

    /// Adds `action` to channel `channel` as a drop from the Browser: the channel becomes the selected one and its
    /// Device Rack shows the new card. A channel that does not take it is left alone and says so.
    pub fn add_dropped(
        &mut self,
        app: &mut AppCtx,
        picker: &mut preset_picker::State,
        channel: usize,
        action: BrowserAction,
    ) {
        let Some(ch) = app.song.channels.get(channel) else {
            return;
        };
        let fits = rows("", ModuleOrdering::Category, crate::ui::rack::takes(ch))
            .into_iter()
            .any(|r| matches!(r, Row::Item(e) if e.action() == action));
        if !fits {
            app.files.flash("That track does not take it");
            return;
        }
        if channel > 0 {
            *app.selected = channel;
        }
        picks::add_pick(picker, app, channel, action);
        if let Some(ch) = app.song.channels.get(channel) {
            self.reveal = ch.rack.modules.len().checked_sub(1);
        }
        self.rack_open = true;
    }

    /// Whether the app may read the internal storage: asked of the system about once a second (a JNI call), and never
    /// where there is no internal storage.
    fn storage_granted(&mut self, now: f64) -> bool {
        if device_storage::root().is_some() && now - self.storage.1 >= 1.0 {
            self.storage = (device_storage::granted(), now);
        }
        self.storage.0
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
                if !self.pins_read {
                    self.pins_read = true;
                    self.pins = app.files.user_dir().map(pins).unwrap_or_default();
                }
                let granted = self.storage_granted(ui.input(|i| i.time));
                let folder = self.folders[slot].last().cloned();
                let key: FilesKey = (
                    tab,
                    folder,
                    app.files.library_changes() + u64::from(granted),
                );
                if self.files.as_ref().is_none_or(|(k, _)| *k != key) {
                    let read = match (tab, key.1.as_deref()) {
                        // The internal storage: read from disk, already sorted.
                        (Tab::Samples, Some(dir)) if on_storage(dir) => {
                            Ok((device_storage::list(dir))
                                .into_iter()
                                .map(|(name, path, is_folder)| SongEntry {
                                    name,
                                    path,
                                    is_folder,
                                    recency: 0,
                                })
                                .collect::<Vec<_>>())
                        }
                        (Tab::Samples, dir) => app.files.sample_library().list(dir),
                        (_, dir) => app.files.library.list(dir),
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
                if let Some(top) = self.folders[slot].last() {
                    items.push(Item::Up(place_name(top, app.files.user_dir())));
                } else if tab == Tab::Samples {
                    // The top: the places (the internal storage, its music folders, the pinned ones), then the app's
                    // own library.
                    if let Some(root) = device_storage::root() {
                        items.push(Item::Section("PLACES"));
                        if self.storage.0 {
                            items.push(Item::Place("Internal storage".into(), root, false));
                            for (name, path) in device_storage::places() {
                                items.push(Item::Place(name, path, false));
                            }
                            for path in &self.pins {
                                let name = place_name(path, None);
                                items.push(Item::Place(name, path.clone(), true));
                            }
                        } else {
                            items.push(Item::Grant);
                        }
                        items.push(Item::Section("LIBRARY"));
                    }
                    if let Some(user) = app.files.user_dir() {
                        items.push(Item::Place("My library".into(), user.to_path_buf(), false));
                    }
                    items.push(Item::AddFolder);
                }
                let entries = self.files.as_ref().map_or(&[][..], |(_, e)| &e[..]);
                for e in entries {
                    items.push(if e.is_folder {
                        Item::Folder(e)
                    } else {
                        Item::File(e)
                    });
                }
                if tab == Tab::Samples && entries.iter().any(|e| !e.is_folder) {
                    items.insert(
                        items.len().min(1),
                        Item::Note("Hold a sample, then drag it onto the playlist"),
                    );
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
        let mut begun: Option<(BrowserAction, String)> = None;
        let mut pin: Option<PathBuf> = None;
        let mut unreadable = false;
        let user = app.files.user_dir().map(std::path::Path::to_path_buf);
        let mut child = ui.new_child(UiBuilder::new().max_rect(list).id(("dock list", tab as u8)));
        child.set_clip_rect(list.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(("dock list scroll", tab as u8))
            .scroll_source(if crate::ui::file_drag::dragging(ui.ctx()) {
                egui::scroll_area::ScrollSource::NONE
            } else {
                egui::scroll_area::ScrollSource::ALL
            })
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
                        Item::Up(name) => {
                            triangle(
                                painter,
                                pos2(r.left() + pad + m.u(0.2), r.center().y),
                                m.u(0.2),
                                Dir::Left,
                                look.text,
                            );
                            let name = ellipsis(
                                painter,
                                name,
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
                        Item::Place(name, path, pinned) => {
                            folder_icon(
                                painter,
                                pos2(r.left() + pad + m.u(0.45), r.center().y),
                                m.u(0.5),
                                look.accent,
                                look.px,
                            );
                            let name = ellipsis(
                                painter,
                                name,
                                &font,
                                look.text,
                                r.width() - pad - m.u(3.0),
                            );
                            painter.text(
                                pos2(at.x + m.u(1.3), at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.text,
                            );
                            if *pinned {
                                pin_icon(
                                    painter,
                                    pos2(r.right() - m.u(0.8), r.center().y),
                                    m.u(0.5),
                                    true,
                                    look,
                                );
                                let pin_r = Rect::from_min_size(
                                    pos2(r.right() - m.u(1.5), r.top()),
                                    vec2(m.u(1.5), r.height()),
                                );
                                let id = egui::Id::new(("dock unpin", path.clone()));
                                if ui.interact(pin_r, id, sense).clicked() {
                                    pin = Some(path.clone());
                                }
                            }
                        }
                        Item::Grant => {
                            folder_icon(
                                painter,
                                pos2(r.left() + pad + m.u(0.45), r.center().y),
                                m.u(0.5),
                                look.accent,
                                look.px,
                            );
                            let name = ellipsis(
                                painter,
                                "Allow access to internal storage",
                                &font,
                                look.accent,
                                r.width() - pad - m.u(1.5),
                            );
                            painter.text(
                                pos2(at.x + m.u(1.3), at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.accent,
                            );
                        }
                        Item::Folder(e) => {
                            let device = on_storage(&e.path);
                            folder_icon(
                                painter,
                                pos2(r.left() + pad + m.u(0.45), r.center().y),
                                m.u(0.5),
                                look.dim,
                                look.px,
                            );
                            let name = ellipsis(
                                painter,
                                &e.name,
                                &font,
                                look.text,
                                r.width() - pad - m.u(if device { 3.0 } else { 1.8 }),
                            );
                            painter.text(
                                pos2(at.x + m.u(1.3), at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.text,
                            );
                            // A folder of the internal storage can be pinned as a place.
                            if device {
                                let on = self.pins.contains(&e.path);
                                pin_icon(
                                    painter,
                                    pos2(r.right() - m.u(0.8), r.center().y),
                                    m.u(0.5),
                                    on,
                                    look,
                                );
                                let pin_r = Rect::from_min_size(
                                    pos2(r.right() - m.u(1.5), r.top()),
                                    vec2(m.u(1.5), r.height()),
                                );
                                let id = egui::Id::new(("dock pin", e.path.clone()));
                                if ui.interact(pin_r, id, sense).clicked() {
                                    pin = Some(e.path.clone());
                                }
                            }
                        }
                        Item::AddFolder => {
                            let plus = pos2(r.left() + pad + m.u(0.2), r.center().y);
                            let arm = m.u(0.24);
                            let stroke = Stroke::new(1.5 * look.px, look.accent);
                            painter.line_segment(
                                [plus - vec2(arm, 0.0), plus + vec2(arm, 0.0)],
                                stroke,
                            );
                            painter.line_segment(
                                [plus - vec2(0.0, arm), plus + vec2(0.0, arm)],
                                stroke,
                            );
                            let name = ellipsis(
                                painter,
                                "Import a folder into the library...",
                                &font,
                                look.accent,
                                r.width() - pad - m.u(1.2),
                            );
                            painter.text(
                                pos2(at.x + m.u(0.8), at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.accent,
                            );
                        }
                        Item::File(e) => {
                            let sample = tab == Tab::Samples;
                            let shift = if sample { m.u(1.3) } else { 0.0 };
                            if sample {
                                wave_icon(
                                    painter,
                                    pos2(r.left() + pad + m.u(0.45), r.center().y),
                                    m.u(0.5),
                                    look.dim,
                                    look.px,
                                );
                            }
                            let name = ellipsis(
                                painter,
                                &e.name,
                                &font,
                                look.text,
                                r.width() - 2.0 * pad - shift,
                            );
                            painter.text(
                                pos2(at.x + shift, at.y),
                                Align2::LEFT_CENTER,
                                name,
                                font.clone(),
                                look.text,
                            );
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
                    // A press on a module row dragged out of the list to the right (a vertical drag scrolls) carries it
                    // to a track or the Device Rack; letting go there adds it.
                    if let Item::Module(entry) = item
                        && active
                        && response.is_pointer_button_down_on()
                        && ui
                            .input(|i| i.pointer.interact_pos())
                            .is_some_and(|p| p.x > list.right() + m.u(DRAG_OUT))
                    {
                        begun = Some((entry.action(), entry.name().to_owned()));
                    }
                    // A long press then a drag on a sample row carries the file to the playlist: where it lands on a
                    // track it makes a clip, and anywhere else a new audio track ([`crate::ui::file_drag`]).
                    // A file of the internal storage is copied into `My Samples/Device` first: a project reads its files from
                    // the user folder.
                    if let Item::File(e) = item
                        && tab == Tab::Samples
                        && active
                        && !crate::ui::file_drag::dragging(ui.ctx())
                        && crate::ui::file_drag::row_dragged(ui, &response, m)
                    {
                        let path = if on_storage(&e.path) {
                            (user.as_deref()).and_then(|u| device_storage::bring_in(u, &e.path))
                        } else {
                            Some(e.path.clone())
                        };
                        match path {
                            Some(path) => crate::ui::file_drag::start(ui.ctx(), &path, &e.name),
                            None => unreadable = true,
                        }
                    }
                    if response.clicked() {
                        click = match item {
                            Item::AddFolder => Some(Click::AddFolder),
                            Item::Module(entry) => Some(Click::Pick(entry.action())),
                            Item::Up(_) => Some(Click::Up),
                            Item::Grant => Some(Click::Grant),
                            Item::Place(_, path, _) => Some(Click::Enter(path.clone())),
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

        if begun.is_some() {
            self.drag = begun;
        }
        if unreadable {
            app.files.flash("Could not read that file");
        }
        if let Some(path) = pin {
            if let Some(at) = self.pins.iter().position(|p| *p == path) {
                self.pins.remove(at);
            } else {
                self.pins.push(path);
            }
            if let Some(user) = user.as_deref() {
                save_pins(user, &self.pins);
            }
        }
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
            Click::Grant => {
                device_storage::ask();
                // Asked again at once on the way back.
                self.storage.1 = f64::NEG_INFINITY;
                None
            }
            Click::AddFolder => {
                // The system folder picker; the folder's audio is copied into `My Samples` after a Yes, and listed.
                app.files.pick_folder();
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
                        app.files.flash(
                            "Select an audio track, or hold a sample and drag it onto the playlist",
                        );
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

        // The grab strip on its top edge: drag up or down to stretch the rack, double tap to give it back its height.
        if self.rack_open {
            let grip = Rect::from_x_y_ranges(
                rect.x_range(),
                rect.top() - m.u(GRIP / 2.0)..=rect.top() + m.u(GRIP / 2.0),
            );
            let drag = ui.interact(grip, egui::Id::new("dock rack grip"), grab_sense(covered));
            if drag.dragged() {
                let body = rect.height() / m.u(1.0) - HEADER_H;
                self.rack_body =
                    (body - drag.drag_delta().y / m.u(1.0)).clamp(RACK_RANGE.0, RACK_RANGE.1);
            } else if drag.double_clicked() {
                self.rack_body = RACK_BODY_H;
            }
            paint_grip(&painter, grip, true, &drag, &look, ui);
        }

        let body = rect.with_min_y(header.bottom());
        if !draw_strip || !self.rack_open || body.height() < 1.0 || name.is_none() {
            return nav;
        }
        let reveal = self.reveal.take();
        let strip = rack::show_strip(state, ui, app, body, selected, clipboard, held, reveal);
        nav.or(strip)
    }
}

/// A grab strip's input: a drag, none while a dialog covers it.
fn grab_sense(covered: bool) -> Sense {
    if covered {
        Sense::hover()
    } else {
        Sense::click_and_drag()
    }
}

/// A grab strip's look: while hovered or dragged an accent line along the edge, always three dots at its middle; the
/// pointer is a resize arrow over it.
fn paint_grip(
    painter: &egui::Painter,
    grip: Rect,
    horizontal: bool,
    response: &egui::Response,
    look: &Look,
    ui: &Ui,
) {
    let live = response.dragged() || widgets::hovered(ui, response);
    if live {
        ui.ctx().set_cursor_icon(if horizontal {
            egui::CursorIcon::ResizeVertical
        } else {
            egui::CursorIcon::ResizeHorizontal
        });
    }
    let tone = if live { look.accent } else { look.dim };
    let c = grip.center();
    let r = look.px * 1.5;
    let step = look.px * 7.0;
    for i in -1..=1 {
        let at = if horizontal {
            pos2(c.x + step * i as f32, c.y)
        } else {
            pos2(c.x, c.y + step * i as f32)
        };
        painter.circle_filled(at, r, tone);
    }
    if live {
        let line = if horizontal {
            [grip.left_center(), grip.right_center()]
        } else {
            [
                grip.right_top() - vec2(look.px, 0.0),
                grip.right_bottom() - vec2(look.px, 0.0),
            ]
        };
        painter.line_segment(line, Stroke::new(2.0 * look.px, look.accent));
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

/// A solid triangle `size` from its centre to its corners, pointing `dir`: Ableton's disclosure arrow.
fn triangle(painter: &egui::Painter, center: Pos2, size: f32, dir: Dir, color: Color32) {
    let (a, b, tip) = match dir {
        Dir::Left => (vec2(size, -size), vec2(size, size), vec2(-size, 0.0)),
        Dir::Right => (vec2(-size, -size), vec2(-size, size), vec2(size, 0.0)),
        Dir::Down => (vec2(-size, -size), vec2(size, -size), vec2(0.0, size)),
    };
    painter.add(Shape::convex_polygon(
        vec![center + a, center + b, center + tip],
        color,
        Stroke::NONE,
    ));
}

/// The arrows of the docks: solid triangles, as Live draws them.
fn chevron(painter: &egui::Painter, center: Pos2, size: f32, dir: Dir, color: Color32, _px: f32) {
    triangle(painter, center, size * 0.8, dir, color);
}

/// A folder: a body with its tab, `size` wide.
fn folder_icon(painter: &egui::Painter, c: Pos2, size: f32, color: Color32, _px: f32) {
    let (w, h) = (size * 0.5, size * 0.36);
    let body = Rect::from_center_size(c + vec2(0.0, h * 0.15), vec2(2.0 * w, 1.6 * h));
    painter.rect_filled(body, size * 0.06, color);
    let tab = Rect::from_min_size(
        pos2(body.left(), body.top() - h * 0.45),
        vec2(w * 0.8, h * 0.5),
    );
    painter.rect_filled(tab, size * 0.05, color);
}

/// An audio file: a few bars of a waveform, `size` high.
fn wave_icon(painter: &egui::Painter, c: Pos2, size: f32, color: Color32, _px: f32) {
    let heights = [0.35, 0.8, 0.55, 1.0, 0.45, 0.7];
    let (step, n) = (size * 0.2, heights.len() as f32);
    for (i, h) in heights.into_iter().enumerate() {
        let x = c.x + (i as f32 - (n - 1.0) / 2.0) * step;
        painter.line_segment(
            [
                pos2(x, c.y - h * size * 0.45),
                pos2(x, c.y + h * size * 0.45),
            ],
            Stroke::new(size * 0.1, color),
        );
    }
}

/// A bookmark: filled in the accent while the folder is pinned as a place.
fn pin_icon(painter: &egui::Painter, c: Pos2, size: f32, on: bool, look: &Look) {
    let (w, h) = (size * 0.28, size * 0.42);
    let points = vec![
        c + vec2(-w, -h),
        c + vec2(w, -h),
        c + vec2(w, h),
        c + vec2(0.0, h * 0.45),
        c + vec2(-w, h),
    ];
    let tone = if on { look.accent } else { look.dim };
    painter.add(Shape::closed_line(
        points,
        Stroke::new((if on { 2.5 } else { 1.5 }) * look.px, tone),
    ));
    if on {
        let top = Rect::from_min_max(c + vec2(-w, -h), c + vec2(w, h * 0.2));
        painter.rect_filled(top, 0.0, look.accent);
    }
}

/// The name a place or folder is listed under: `Internal storage` for the storage's root, `Library` for the user
/// folder, else the folder's own name.
fn place_name(path: &std::path::Path, user: Option<&std::path::Path>) -> String {
    if user.is_some_and(|u| u == path) {
        return "Library".into();
    }
    if device_storage::root().is_some_and(|r| r == path) {
        return "Internal storage".into();
    }
    path.file_name()
        .map_or_else(|| "Folder".into(), |n| n.to_string_lossy().into_owned())
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
