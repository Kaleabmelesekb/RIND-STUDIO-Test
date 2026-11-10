// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rind's picture and its hit regions, in logical pixels: one screen, drawn to the Rind Audio Labs design (a 1278 x 832
//! picture; every coordinate below is in that picture's pixels and `s` brings it to the window). A wooden frame and a
//! cream top bar (polyphony, voices, part select, solo, sync); the two oscillators of the chosen part either side of
//! its amplitude envelope; the filter of the part and the filter control; the display (patch bar, menu, the effect
//! list, and the arpeggiator or an effect's controls); the mixer with its meters; two modulation envelopes, two LFOs
//! and two free modulation panels; and the bend range, the wheels, a five-octave keyboard, mono legato, portamento
//! and its mode.
//!
//! Everything is driven by the parameter table ([`crate::ids::SPECS`]); a page is a list of parameter indices.

use os_ui::{Canvas, Color, Font, Rect};

use crate::ids::{self, Fmt, Kind as Range, SPECS};

/// The editor's size at scale 1: the proportions of the 1278 x 832 design.
pub const W: u32 = 800;
pub const H: u32 = 521;

/// Drag length in px for the whole range of a knob.
pub const KNOB_PX: f32 = 150.0;
/// A spin box counts one step per this many px of drag.
pub const SPIN_STEP_PX: f32 = 10.0;

/// The drag length (px) over the whole range of spin box `p`: a step is [`SPIN_STEP_PX`] (a range that is not whole
/// takes as long as a knob).
pub fn spin_px(p: usize) -> f32 {
    match ids::SPECS[p].kind {
        ids::Kind::Int(lo, hi) if hi > lo => (hi - lo) as f32 * SPIN_STEP_PX,
        _ => KNOB_PX,
    }
}

/// A design pixel (of 1278 x 832) in logical pixels.
const K: f32 = 800.0 / 1278.0;

fn s(v: f32) -> f32 {
    v * K
}

/// A rect given in design pixels.
fn rr(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(s(x), s(y), s(w), s(h))
}

// The design's colours: the wooden frame, graphite and brown panels, the cream bar and the blue-grey display.
const BG: Color = Color::rgb(0x19191b);
const WOOD: Color = Color::rgb(0xa8693f);
const WOOD_DARK: Color = Color::rgb(0x84502d);
const WOOD_LIGHT: Color = Color::rgb(0xbd8157);
const BAR_TOP: Color = Color::rgb(0xfdf8f0);
const BAR_BOTTOM: Color = Color::rgb(0xe9dfce);
const BAR_TEXT: Color = Color::rgb(0x4d453e);
const BROWN: Color = Color::rgb(0x5a4f49);
const BROWN_HI: Color = Color::rgb(0x675d57);
const GRAPHITE: Color = Color::rgb(0x353535);
const GRAPHITE_HI: Color = Color::rgb(0x424242);
const HEAD: Color = Color::rgb(0x1d150e);
const HEAD_TEXT: Color = Color::rgb(0xf2e8d8);
const TEXT: Color = Color::rgb(0xf0e8dc);
const TEXT_DIM: Color = Color::rgb(0xb8aea2);
const SLOT: Color = Color::rgb(0x0b0b0b);
const EDGE: Color = Color::rgb(0x161616);
const TICK: Color = Color::rgb(0xd6cec2);
const SHADOW: Color = Color::rgb(0x000000).with_alpha(0x80);
const SKIRT: Color = Color::rgb(0x101010);
const SKIRT_HI: Color = Color::rgb(0x3a3a3a);
const CAP_LO: Color = Color::rgb(0x6f6f6f);
const CAP_MID: Color = Color::rgb(0xa8a8a8);
const CAP_HI: Color = Color::rgb(0xe6e6e6);
const FIELD: Color = Color::rgb(0xa2b6c0);
const FIELD_EDGE: Color = Color::rgb(0x56707c);
const FIELD_TEXT: Color = Color::rgb(0x1c2a33);
const ARROW: Color = Color::rgb(0xe9e1d4);
const LAMP_ON: Color = Color::rgb(0xfffdf6);
const LAMP_GLOW: Color = Color::rgb(0xffe7a6).with_alpha(0x90);
const LAMP_OFF: Color = Color::rgb(0x2a2a2a);
const CHIP: Color = Color::rgb(0xefd6b4);
const CHIP_TEXT: Color = Color::rgb(0x5a3b20);
const CHIP_DIM: Color = Color::rgb(0x6b5f58);
const CASE: Color = Color::rgb(0x2a2a2e);
const CASE_HI: Color = Color::rgb(0x45454b);
const PAPER: Color = Color::rgb(0xc4d6dc);
const PAPER_HI: Color = Color::rgb(0xd6e3e7);
const BLUE: Color = Color::rgb(0x4b6586);
const BLUE_DARK: Color = Color::rgb(0x394f70);
const INK: Color = Color::rgb(0x34507c);
const INK_DIM: Color = Color::rgb(0x6d86a6);
const WHITE: Color = Color::rgb(0xf5f9fc);
const SHADE: Color = Color::rgb(0x000000).with_alpha(0xaa);
const VEIL: Color = Color::rgb(0x19191b).with_alpha(0xb8);
const GOLD: Color = Color::rgb(0xffc86b);
const KEY_WHITE: Color = Color::rgb(0xf4f4f4);
const KEY_SHADE: Color = Color::rgb(0xcfcfcf);
const KEY_DOWN: Color = Color::rgb(0xf0b070);
const KEY_BLACK: Color = Color::rgb(0x161616);
const LAMP_RED: Color = Color::rgb(0xe0503a);
const LAMP_RED_OFF: Color = Color::rgb(0x4a2a22);
const LAMP_YELLOW: Color = Color::rgb(0xf4d24a);
const LAMP_YELLOW_OFF: Color = Color::rgb(0x4a432a);
const LAMP_GREEN: Color = Color::rgb(0x7be07a);
const LAMP_GREEN_OFF: Color = Color::rgb(0x2a3a2a);

const LABEL: Font = Font::new(7).bold();
const VALUE: Font = Font::new(8).bold();
const TITLE: Font = Font::new(9).bold();
const SMALL: Font = Font::new(6).bold();
const BAR: Font = Font::new(9).bold().italic();

/// What a tap on a navigation control changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    /// The part whose oscillators, envelope and filter the screen shows.
    Part(usize),
    /// The patch slot (1-4) the patch bar shows.
    Tab(usize),
    /// The effect (or the arpeggiator) the display shows.
    Fx(usize),
    /// The arpeggiator's steps 1-8 or 9-16.
    Page(usize),
    /// The display's menu opens or closes.
    Menu,
    /// The next (true) or previous patch.
    Patch(bool),
}

/// What the screen is showing, and the list open, if any (the parameter it picks).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ui {
    pub part: usize,
    pub tab: usize,
    pub fx: usize,
    pub page: usize,
    pub popup: Option<usize>,
    /// The display's menu is open.
    pub menu: bool,
    /// The keys of the on-screen keyboard held down, one bit per note (any number at once).
    pub keys: u128,
}

impl Ui {
    pub fn apply(&mut self, nav: Nav) {
        match nav {
            Nav::Part(i) => self.part = i.min(1),
            Nav::Tab(i) => self.tab = i.min(3),
            Nav::Fx(i) => self.fx = i.min(FX_NAMES.len() - 1),
            Nav::Page(i) => self.page = i.min(1),
            Nav::Menu => self.menu = !self.menu,
            Nav::Patch(_) => {}
        }
    }
}

/// What the display's menu offers, in order.
pub const MENU_ITEMS: [&str; 4] = ["Reload Patch", "Init Patch", "Previous Patch", "Next Patch"];

const FX_NAMES: [&str; 8] = [
    "ARPEG", "DISTORT", "PHASER", "CHORUS", "EQ", "DELAY", "REVERB", "COMPRESS",
];
const FX_ON: [usize; 8] = [
    ids::ARP_ON,
    ids::DIST_ON,
    ids::PHS_ON,
    ids::CHO_ON,
    ids::EQ_ON,
    ids::DLY_ON,
    ids::REV_ON,
    ids::CMP_ON,
];
const FX_PARAMS: [&[usize]; 8] = [
    &[],
    &[
        ids::DIST_TYPE,
        ids::DIST_DRIVE,
        ids::DIST_TONE,
        ids::DIST_MIX,
    ],
    &[
        ids::PHS_RATE,
        ids::PHS_DEPTH,
        ids::PHS_FREQ,
        ids::PHS_FB,
        ids::PHS_SPREAD,
        ids::PHS_MIX,
    ],
    &[
        ids::CHO_RATE,
        ids::CHO_DEPTH,
        ids::CHO_DELAY,
        ids::CHO_FB,
        ids::CHO_MIX,
    ],
    &[
        ids::EQ_BASS_F,
        ids::EQ_BASS_G,
        ids::EQ_TREB_F,
        ids::EQ_TREB_G,
    ],
    &[
        ids::DLY_SYNC,
        ids::DLY_L,
        ids::DLY_R,
        ids::DLY_DIV_L,
        ids::DLY_DIV_R,
        ids::DLY_FB,
        ids::DLY_PP,
        ids::DLY_LP,
        ids::DLY_HP,
        ids::DLY_SMEAR,
        ids::DLY_MIX,
    ],
    &[
        ids::REV_PRE,
        ids::REV_DAMP,
        ids::REV_SIZE,
        ids::REV_WIDTH,
        ids::REV_MIX,
    ],
    &[
        ids::CMP_THR,
        ids::CMP_RATIO,
        ids::CMP_ATT,
        ids::CMP_REL,
        ids::CMP_GAIN,
    ],
];

/// What a hit region is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Drag vertically (the cursor held).
    Knob,
    /// A named range: a tap steps to the next name, or opens the list when there are many.
    Cycle,
    Toggle,
    /// A whole-number or named box with arrows: a press in its upper half steps up, in its lower half down.
    Spin,
    /// A vertical fader: a press sets, a drag moves.
    Fader,
    /// A number cell: a vertical drag changes it.
    Num,
    /// A lamp or switch on a continuous parameter: lit (or, when `true`, lit at its low end) it flips between the ends.
    Switch(bool),
    /// A filter's input: A, B or both (a tap on the upper half steps forward, the lower back).
    Route,
    /// A wheel: a vertical drag (the pitch wheel springs back).
    Wheel,
    /// A key of the keyboard (the MIDI note).
    Key(u8),
    /// Part, tab, effect, page, menu or patch buttons.
    Nav(Nav),
    /// Name number `i` in the open drop-down list.
    Item(usize),
    /// Entry `i` of the display's open menu.
    MenuItem(usize),
    /// Anywhere else while a list is open: it closes.
    Dismiss,
}

/// A control's parameter index (into the table), kind and rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub param: usize,
    pub kind: Kind,
    pub r: Rect,
}

impl Hit {
    /// True for a control of a parameter (not chrome, a key or a list).
    pub fn is_param(&self) -> bool {
        !matches!(
            self.kind,
            Kind::Nav(_) | Kind::Item(_) | Kind::MenuItem(_) | Kind::Dismiss | Kind::Key(_)
        )
    }
}

/// The values the editor shows.
pub trait Values {
    /// Normalised.
    fn norm(&self, p: usize) -> f32;
    fn plain(&self, p: usize) -> f32;
    fn text(&self, p: usize) -> String;
    /// The output meters' linear levels (left, right).
    fn levels(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
    /// The voices sounding.
    fn voices(&self) -> usize {
        0
    }
}

/// The kind of control a parameter gets by its range.
pub fn widget(p: usize) -> Kind {
    match SPECS[p].fmt {
        Fmt::Names(n) if n.len() == 2 && n[0] == "Off" => Kind::Toggle,
        Fmt::Names(_) => Kind::Cycle,
        _ => Kind::Knob,
    }
}

/// True for a parameter that steps in whole units.
pub fn is_int(p: usize) -> bool {
    matches!(SPECS[p].kind, Range::Int(..))
}

/// True for a parameter that runs through zero (drawn from its middle).
pub fn bipolar(p: usize) -> bool {
    match SPECS[p].kind {
        Range::Lin(lo, hi) | Range::Skew(lo, hi, _) => lo < 0.0 && hi > 0.0,
        Range::Int(lo, hi) => lo < 0 && hi > 0,
    }
}

/// The names of a selector, and the value the first stands for.
pub fn choices(p: usize) -> (&'static [&'static str], i32) {
    match (SPECS[p].fmt, SPECS[p].kind) {
        (Fmt::Names(n), Range::Int(lo, _)) => (n, lo),
        (Fmt::Names(n), _) => (n, 0),
        _ => (&[], 0),
    }
}

/// True for a selector that opens a list (four names or more) when tapped: a drop-down. (A spin box steps instead.)
pub fn dropdown(p: usize) -> bool {
    widget(p) == Kind::Cycle && choices(p).0.len() >= 4
}

/// The normalised value a press at `y` sets on a fader in `r`.
pub fn fader_norm(r: Rect, y: f32) -> f32 {
    (1.0 - (y - r.y - FADER_PAD) / (r.h - 2.0 * FADER_PAD)).clamp(0.0, 1.0)
}

pub const FADER_PAD: f32 = 5.0;
/// A drag over this many px runs a number cell through its range.
pub const NUM_PX: f32 = 120.0;
/// A drag over this many px runs a wheel through its range.
pub const WHEEL_PX: f32 = 70.0;

/// A filter's routing as the input select shows it: its two input levels as A, B, both, or R (a mix of its own).
pub fn route_name(a: f32, b: f32) -> &'static str {
    match (a >= 0.99, a <= 0.01, b >= 0.99, b <= 0.01) {
        (true, _, _, true) => "A",
        (_, true, true, _) => "B",
        (true, _, true, _) => "AB",
        _ => "R",
    }
}

/// The input levels after stepping `from` (as [`route_name`] reads it) by `step` through A, B, AB.
pub fn route_step(a: f32, b: f32, step: i32) -> (f32, f32) {
    const ROUTES: [(f32, f32); 3] = [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let now = match route_name(a, b) {
        "A" => 0,
        "B" => 1,
        "AB" => 2,
        _ => return ROUTES[if step >= 0 { 0 } else { 2 }],
    };
    ROUTES[(now as i32 + step).rem_euclid(3) as usize]
}

/// The parameter that mirrors `p` on the other part while Part Sync is on: the same control of the other oscillator
/// pair, amplitude envelope or filter.
pub fn mirror(p: usize) -> Option<usize> {
    let osc_end = ids::OSC_BASE + 4 * ids::OSC_STRIDE;
    if p >= ids::OSC_BASE && p < osc_end {
        let (n, f) = (
            (p - ids::OSC_BASE) / ids::OSC_STRIDE,
            (p - ids::OSC_BASE) % ids::OSC_STRIDE,
        );
        return Some(osc_param((n + 2) % 4, f));
    }
    let part_end = ids::PART_BASE + 2 * ids::PART_STRIDE;
    if p >= ids::PART_BASE && p < part_end {
        let (n, f) = (
            (p - ids::PART_BASE) / ids::PART_STRIDE,
            (p - ids::PART_BASE) % ids::PART_STRIDE,
        );
        return (f >= ids::part::ATT).then(|| part_param(1 - n, f));
    }
    let filt_end = ids::FILT_BASE + 2 * ids::FILT_STRIDE;
    if p >= ids::FILT_BASE && p < filt_end {
        let (n, f) = (
            (p - ids::FILT_BASE) / ids::FILT_STRIDE,
            (p - ids::FILT_BASE) % ids::FILT_STRIDE,
        );
        let shared = [
            ids::filt::TYPE,
            ids::filt::SLOPE,
            ids::filt::CUTOFF,
            ids::filt::RES,
            ids::filt::DRIVE,
            ids::filt::KEY,
        ];
        return shared.contains(&f).then(|| filt_param(1 - n, f));
    }
    None
}

// ---- the matrix rows --------------------------------------------------------------------------------------------

/// Where a matrix slot's source comes from on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotSrc {
    /// The panel's own source (an envelope or LFO): a row shows only its target and amount.
    Fixed(usize),
    /// The slot has the source drop-down of its group.
    Free,
    /// The second row of a group: its source is the first slot's.
    Follow(usize),
}

fn slot_src(slot: usize) -> SlotSrc {
    match slot {
        0 | 1 => SlotSrc::Fixed(ids::msrc::ENV_3),
        2 | 3 => SlotSrc::Fixed(ids::msrc::ENV_4),
        4 | 5 => SlotSrc::Fixed(ids::msrc::LFO_1),
        6 | 7 => SlotSrc::Fixed(ids::msrc::LFO_2),
        s if s % 2 == 0 => SlotSrc::Free,
        s => SlotSrc::Follow(s - 1),
    }
}

fn mat_param(slot: usize, field: usize) -> usize {
    ids::MAT_BASE + slot * ids::MAT_STRIDE + field
}

/// For a target or amount of a matrix row whose source the screen decides: the source parameter and the value it must
/// have for the row to mean what it shows.
pub fn row_source(v: &dyn Values, p: usize) -> Option<(usize, f32)> {
    let end = ids::MAT_BASE + 16 * ids::MAT_STRIDE;
    if p < ids::MAT_BASE || p >= end {
        return None;
    }
    let (slot, field) = (
        (p - ids::MAT_BASE) / ids::MAT_STRIDE,
        (p - ids::MAT_BASE) % ids::MAT_STRIDE,
    );
    if field == ids::mat::SRC {
        return None;
    }
    let src = mat_param(slot, ids::mat::SRC);
    match slot_src(slot) {
        SlotSrc::Fixed(want) => Some((src, want as f32)),
        SlotSrc::Follow(first) => Some((src, v.plain(mat_param(first, ids::mat::SRC)))),
        SlotSrc::Free => None,
    }
}

/// The source parameter that follows `p` when `p` is the source drop-down of a group (set together with it).
pub fn partner(p: usize) -> Option<usize> {
    let end = ids::MAT_BASE + 16 * ids::MAT_STRIDE;
    if p < ids::MAT_BASE || p >= end || (p - ids::MAT_BASE) % ids::MAT_STRIDE != ids::mat::SRC {
        return None;
    }
    let slot = (p - ids::MAT_BASE) / ids::MAT_STRIDE;
    (slot_src(slot) == SlotSrc::Free).then(|| mat_param(slot + 1, ids::mat::SRC))
}

/// True for a source parameter the screen sets for the rows (it has no control of its own).
pub fn derived_source(p: usize) -> bool {
    let end = ids::MAT_BASE + 16 * ids::MAT_STRIDE;
    p >= ids::MAT_BASE
        && p < end
        && (p - ids::MAT_BASE) % ids::MAT_STRIDE == ids::mat::SRC
        && slot_src((p - ids::MAT_BASE) / ids::MAT_STRIDE) != SlotSrc::Free
}

// ---- layout ----------------------------------------------------------------------------------------------------

/// How a control is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    /// A knob of this radius in design pixels, its label under it (none when the label is empty).
    Knob(f32),
    /// A light field with its value; with arrows beside it when `true`.
    Field(bool),
    /// A dark box with the wave or filter shape and arrows beside it.
    Icon,
    /// A blue bar of the display: a name, with a chevron or arrows.
    Bar(bool),
    Check,
    /// A lamp: a lit square.
    Lamp,
    /// The 12 / 24 dB slide switch.
    Slide,
    Fader,
    /// A cell of the arpeggiator's table.
    Num,
    /// A navigation button with its caption: in the top bar, or of the display.
    TopBtn(&'static str),
    Tab(&'static str),
    /// The patch bar's previous (false) and next (true) buttons.
    Step(bool),
    /// The small blue triangle button of the display.
    Menu(&'static str),
    Wheel,
    Key(bool),
}

struct Item {
    param: usize,
    kind: Kind,
    r: Rect,
    look: Look,
    /// The caption: above a bar or switch, below a knob; empty for none.
    label: &'static str,
    /// Drawn selected (a navigation button).
    on: bool,
    /// Drawn in the display's colours.
    lcd: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
    Right,
}

/// Text that is only drawn, at (`x`, `y` the middle line) in design pixels.
struct Text {
    x: f32,
    y: f32,
    s: &'static str,
    font: Font,
    color: Color,
    align: Align,
}

/// A panel of the front: its title, rect, and look.
#[derive(Clone, Copy, PartialEq)]
enum Skin {
    Brown,
    Graphite,
}

struct Panel {
    title: String,
    r: Rect,
    skin: Skin,
    /// The small menu glyph at the right of the header.
    glyph: bool,
}

#[derive(Default)]
struct Page {
    panels: Vec<Panel>,
    items: Vec<Item>,
    texts: Vec<Text>,
    /// New items and texts are drawn in the display's colours.
    lcd: bool,
}

impl Page {
    fn frame(
        &mut self,
        title: impl Into<String>,
        skin: Skin,
        glyph: bool,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        self.panels.push(Panel {
            title: title.into(),
            r: rr(x, y, w, h),
            skin,
            glyph,
        });
    }

    fn put(&mut self, x: f32, y: f32, s: &'static str, align: Align, font: Font, color: Color) {
        self.texts.push(Text {
            x,
            y,
            s,
            font,
            color,
            align,
        });
    }

    fn ink(&self) -> Color {
        if self.lcd { INK } else { TEXT }
    }

    /// A caption, left-aligned at `x`, its middle at `y`.
    fn text(&mut self, x: f32, y: f32, s: &'static str) {
        let c = self.ink();
        self.put(x, y, s, Align::Left, LABEL, c);
    }

    fn text_r(&mut self, x: f32, y: f32, s: &'static str) {
        let c = self.ink();
        self.put(x, y, s, Align::Right, LABEL, c);
    }

    fn add(&mut self, param: usize, kind: Kind, r: Rect, look: Look, label: &'static str) {
        self.items.push(Item {
            param,
            kind,
            r,
            look,
            label,
            on: false,
            lcd: self.lcd,
        });
    }

    /// A knob centred at (`cx`, `cy`) in design pixels with a hit region `hw` design pixels either side and from
    /// `rad + 4` above to under the label.
    fn knob(&mut self, p: usize, cx: f32, cy: f32, rad: f32, hw: f32, label: &'static str) {
        let below = if label.is_empty() {
            rad + 4.0
        } else {
            rad + 20.0
        };
        let r = rr(cx - hw, cy - rad - 4.0, 2.0 * hw, rad + 4.0 + below);
        self.add(p, Kind::Knob, r, Look::Knob(rad), label);
    }

    /// A box with arrows beside it: a whole-number or short named parameter that steps.
    fn spin(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32, look: Look) {
        self.add(p, Kind::Spin, rr(x, y, w, h), look, "");
    }

    /// A drop-down field (a name; a list opens on a tap).
    fn pick(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32) {
        self.add(p, Kind::Cycle, rr(x, y, w, h), Look::Field(false), "");
    }

    fn lamp(&mut self, p: usize, kind: Kind, x: f32, y: f32, w: f32, h: f32) {
        self.add(p, kind, rr(x, y, w, h), Look::Lamp, "");
    }

    /// A parameter in a cell of design pixels on the display (a knob with its label under it; a bar, switch or arrows
    /// with its label above).
    fn cell(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32) {
        self.cell_as(p, short(p), x, y, w, h);
    }

    fn cell_as(&mut self, p: usize, label: &'static str, x: f32, y: f32, w: f32, h: f32) {
        let cx = x + w / 2.0;
        match widget(p) {
            Kind::Knob if is_spin(p) => {
                self.add(p, Kind::Spin, rr(x, y, w, h), Look::Bar(true), label);
            }
            Kind::Knob => self.knob(p, cx, y + 20.0, 15.0, w / 2.0, label),
            Kind::Cycle => self.add(p, Kind::Cycle, rr(x, y, w, h), Look::Bar(false), label),
            _ => self.add(p, Kind::Toggle, rr(x, y, w, h), Look::Lamp, label),
        }
    }

    /// A target row of a matrix slot, its middle at `cy`: its amount as a small knob and its target as a drop-down.
    fn row(&mut self, slot: usize, x: f32, cy: f32, w: f32) {
        self.knob(mat_param(slot, ids::mat::AMT), x + 33.0, cy, 9.0, 13.0, "");
        self.pick(
            mat_param(slot, ids::mat::DST),
            x + 52.0,
            cy - 11.0,
            w - 62.0,
            22.0,
        );
    }

    fn nav(&mut self, nav: Nav, look: Look, on: bool, x: f32, y: f32, w: f32, h: f32) {
        self.items.push(Item {
            param: 0,
            kind: Kind::Nav(nav),
            r: rr(x, y, w, h),
            look,
            label: "",
            on,
            lcd: self.lcd,
        });
    }
}

/// True for a parameter drawn as a spin box (a small whole number or short name: octave, note, voices, polyphony,
/// bend, steps, wrap).
fn is_spin(p: usize) -> bool {
    use ids::osc;
    let osc_at = |f: usize| (0..4).any(|n| p == ids::OSC_BASE + n * ids::OSC_STRIDE + f);
    p == ids::POLY
        || p == ids::BEND
        || p == ids::ARP_OCT
        || p == ids::ARP_STEPS
        || p == ids::ARP_WRAP
        || osc_at(osc::OCT)
        || osc_at(osc::SEMI)
        || osc_at(osc::VOICES)
}

fn osc_param(n: usize, f: usize) -> usize {
    ids::OSC_BASE + n * ids::OSC_STRIDE + f
}

fn part_param(n: usize, f: usize) -> usize {
    ids::PART_BASE + n * ids::PART_STRIDE + f
}

fn filt_param(n: usize, f: usize) -> usize {
    ids::FILT_BASE + n * ids::FILT_STRIDE + f
}

fn lfo_param(n: usize, f: usize) -> usize {
    ids::LFO_BASE + n * ids::LFO_STRIDE + f
}

fn env_param(n: usize, f: usize) -> usize {
    ids::ENV_BASE + n * ids::ENV_STRIDE + f
}

const OSC_TITLES: [&str; 4] = [
    "OSCILLATOR A1",
    "OSCILLATOR A2",
    "OSCILLATOR B1",
    "OSCILLATOR B2",
];

/// One oscillator panel at (`x`, `y`), 479 x 143 design pixels.
fn osc_panel(g: &mut Page, n: usize, x: f32, y: f32) {
    use ids::osc;
    g.frame(OSC_TITLES[n], Skin::Brown, true, x, y, 479.0, 143.0);
    g.text(x + 32.0, y + 30.0, "PITCH");
    g.text(x + 12.0, y + 52.0, "OCTAVE");
    g.spin(
        osc_param(n, osc::OCT),
        x + 75.0,
        y + 43.0,
        48.0,
        20.0,
        Look::Field(true),
    );
    g.text(x + 12.0, y + 79.0, "NOTE");
    g.spin(
        osc_param(n, osc::SEMI),
        x + 75.0,
        y + 70.0,
        48.0,
        20.0,
        Look::Field(true),
    );
    g.text(x + 12.0, y + 105.0, "FINE");
    g.knob(osc_param(n, osc::FINE), x + 60.0, y + 118.0, 12.0, 20.0, "");
    let knobs = [
        (osc::VOL, 166.0, "VOLUME"),
        (osc::PHASE, 232.0, "PHASE"),
        (osc::DETUNE, 297.0, "DETUNE"),
        (osc::STEREO, 364.0, "STEREO"),
        (osc::PAN, 429.0, "PAN"),
    ];
    for (f, dx, label) in knobs {
        g.knob(osc_param(n, f), x + dx, y + 61.0, 19.0, 32.0, label);
    }
    let by = y + 122.0;
    g.text(x + 107.0, by, "INV");
    g.add(
        osc_param(n, osc::INV),
        Kind::Toggle,
        rr(x + 134.0, by - 10.0, 26.0, 20.0),
        Look::Check,
        "",
    );
    g.text(x + 172.0, by, "WAVE");
    g.spin(
        osc_param(n, osc::WAVE),
        x + 214.0,
        by - 10.0,
        58.0,
        20.0,
        Look::Icon,
    );
    g.text(x + 284.0, by, "VOICES");
    g.spin(
        osc_param(n, osc::VOICES),
        x + 335.0,
        by - 10.0,
        50.0,
        20.0,
        Look::Field(true),
    );
    g.text(x + 392.0, by, "RETRIG");
    // Retrigger: lit when the phase starts the same every note (no random phase).
    g.lamp(
        osc_param(n, osc::RAND),
        Kind::Switch(true),
        x + 440.0,
        by - 11.0,
        29.0,
        22.0,
    );
}

fn envelope_faders(g: &mut Page, x: f32, y: f32, pitch: f32, params: [usize; 4]) {
    for (i, letter) in ["A", "D", "S", "R"].into_iter().enumerate() {
        let cx = x + pitch * i as f32;
        g.put(cx, y, letter, Align::Center, LABEL, TEXT);
        g.add(
            params[i],
            Kind::Fader,
            rr(cx - 15.0, y + 12.0, 30.0, 80.0),
            Look::Fader,
            "",
        );
    }
}

/// The filter panel of the part (filter `n`) and the filter control under it.
fn filter_panels(g: &mut Page, n: usize) {
    use ids::filt;
    let name = ["FILTER A", "FILTER B"][n];
    g.frame(name, Skin::Brown, true, 43.0, 190.0, 307.0, 167.0);
    g.text(55.0, 258.0, "INPUT SELECT");
    g.add(
        filt_param(n, filt::IN_A),
        Kind::Route,
        rr(152.0, 246.0, 44.0, 24.0),
        Look::Field(true),
        "",
    );
    g.put(250.0, 235.0, "FILTER TYPE", Align::Center, LABEL, TEXT);
    g.spin(
        filt_param(n, filt::TYPE),
        231.0,
        246.0,
        52.0,
        24.0,
        Look::Icon,
    );
    g.put(307.0, 252.0, "12 dB", Align::Left, LABEL, TEXT);
    g.add(
        filt_param(n, filt::SLOPE),
        Kind::Toggle,
        rr(305.0, 264.0, 24.0, 42.0),
        Look::Slide,
        "",
    );
    g.put(307.0, 316.0, "24 dB", Align::Left, LABEL, TEXT);
    g.knob(
        filt_param(n, filt::CUTOFF),
        85.0,
        302.0,
        22.0,
        36.0,
        "CUTOFF",
    );
    g.knob(
        filt_param(n, filt::RES),
        157.0,
        302.0,
        22.0,
        36.0,
        "RESONANCE",
    );
    g.knob(
        filt_param(n, filt::DRIVE),
        248.0,
        302.0,
        22.0,
        36.0,
        "DRIVE",
    );

    // The filter control: the global filter (its own cutoff, resonance and key track, apart from filters A and B) and the
    // warm drive.
    g.frame(
        "FILTER CONTROL",
        Skin::Graphite,
        false,
        43.0,
        363.0,
        307.0,
        124.0,
    );
    g.knob(ids::GF_CUTOFF, 85.0, 430.0, 20.0, 36.0, "CUTOFF");
    g.knob(ids::GF_RES, 167.0, 432.0, 24.0, 40.0, "RESONANCE");
    g.put(245.0, 408.0, "WARM", Align::Center, LABEL, TEXT);
    g.put(245.0, 420.0, "DRIVE", Align::Center, LABEL, TEXT);
    g.lamp(ids::WARM, Kind::Toggle, 231.0, 435.0, 30.0, 22.0);
    g.knob(ids::GF_KEY, 304.0, 442.0, 16.0, 30.0, "KEY TRACK");
}

fn mod_env_panel(g: &mut Page, n: usize, x: f32, y: f32) {
    g.frame(
        format!("MOD ENV {}", n + 1),
        Skin::Graphite,
        true,
        x,
        y,
        192.0,
        208.0,
    );
    envelope_faders(
        g,
        x + 26.0,
        y + 52.0,
        39.0,
        [
            env_param(n, 0),
            env_param(n, 1),
            env_param(n, 2),
            env_param(n, 3),
        ],
    );
    g.row(n * 2, x, y + 159.0, 192.0);
    g.row(n * 2 + 1, x, y + 193.0, 192.0);
}

fn lfo_panel(g: &mut Page, n: usize, x: f32, y: f32) {
    use ids::lfo;
    g.frame(
        format!("LFO {}", n + 1),
        Skin::Graphite,
        true,
        x,
        y,
        192.0,
        208.0,
    );
    g.text(x + 9.0, y + 40.0, "WAVE");
    g.spin(
        lfo_param(n, lfo::WAVE),
        x + 50.0,
        y + 30.0,
        58.0,
        21.0,
        Look::Icon,
    );
    g.text(x + 118.0, y + 40.0, "FREE");
    // Free-running: lit when the LFO is not restarted by each note.
    g.lamp(
        lfo_param(n, lfo::RETRIG),
        Kind::Switch(true),
        x + 154.0,
        y + 29.0,
        29.0,
        22.0,
    );
    let knobs = [
        (lfo::RATE, 30.0, "RATE"),
        (lfo::GAIN, 94.0, "GAIN"),
        (lfo::OFFSET, 158.0, "OFFSET"),
    ];
    for (f, dx, label) in knobs {
        g.knob(lfo_param(n, f), x + dx, y + 89.0, 17.0, 31.0, label);
    }
    g.row(4 + n * 2, x, y + 159.0, 192.0);
    g.row(5 + n * 2, x, y + 193.0, 192.0);
}

fn misc_panel(g: &mut Page, n: usize, x: f32, y: f32) {
    g.frame(
        format!("MISC {}", n + 1),
        Skin::Graphite,
        true,
        x,
        y,
        192.0,
        208.0,
    );
    for group in 0..2 {
        let first = 8 + n * 4 + group * 2;
        let top = y + 34.0 + group as f32 * 94.0;
        g.text(x + 6.0, top, "SOURCE");
        g.pick(
            mat_param(first, ids::mat::SRC),
            x + 64.0,
            top - 10.0,
            120.0,
            21.0,
        );
        g.row(first, x, top + 30.0, 192.0);
        g.row(first + 1, x, top + 64.0, 192.0);
    }
}

/// The fixed part of the screen.
fn front(g: &mut Page, ui: &Ui) {
    // The top bar.
    g.put(88.0, 19.0, "Polyphony", Align::Left, BAR, BAR_TEXT);
    g.spin(ids::POLY, 188.0, 9.0, 42.0, 21.0, Look::Field(true));
    g.put(277.0, 19.0, "Voices", Align::Left, BAR, BAR_TEXT);
    g.put(470.0, 19.0, "Part Select", Align::Left, BAR, BAR_TEXT);
    g.nav(
        Nav::Part(0),
        Look::TopBtn("PART A"),
        ui.part == 0,
        576.0,
        8.0,
        61.0,
        23.0,
    );
    g.nav(
        Nav::Part(1),
        Look::TopBtn("PART B"),
        ui.part == 1,
        643.0,
        8.0,
        64.0,
        23.0,
    );
    g.put(760.0, 19.0, "Solo", Align::Left, BAR, BAR_TEXT);
    g.lamp(
        [ids::SOLO_A, ids::SOLO_B][ui.part],
        Kind::Toggle,
        803.0,
        9.0,
        28.0,
        21.0,
    );
    g.put(878.0, 19.0, "Sync", Align::Left, BAR, BAR_TEXT);
    g.lamp(ids::PART_SYNC, Kind::Toggle, 927.0, 9.0, 28.0, 21.0);

    // The oscillators of the part and its amplitude envelope.
    let (a, b) = (ui.part * 2, ui.part * 2 + 1);
    osc_panel(g, a, 43.0, 42.0);
    osc_panel(g, b, 757.0, 42.0);
    g.frame(
        format!("AMP ENV {}", ["A", "B"][ui.part]),
        Skin::Brown,
        true,
        528.0,
        42.0,
        222.0,
        143.0,
    );
    envelope_faders(
        g,
        583.0,
        80.0,
        36.3,
        [
            part_param(ui.part, ids::part::ATT),
            part_param(ui.part, ids::part::DEC),
            part_param(ui.part, ids::part::SUS),
            part_param(ui.part, ids::part::REL),
        ],
    );

    filter_panels(g, ui.part);

    // The mixer.
    g.frame("MIXER", Skin::Graphite, false, 930.0, 192.0, 305.0, 178.0);
    g.put(977.0, 225.0, "MIX A", Align::Center, LABEL, TEXT);
    g.put(1022.0, 225.0, "MIX B", Align::Center, LABEL, TEXT);
    g.put(1090.0, 225.0, "MAIN VOL", Align::Center, LABEL, TEXT);
    let faders = [
        (part_param(0, ids::part::LEVEL), 977.0),
        (part_param(1, ids::part::LEVEL), 1022.0),
        (ids::MASTER_VOL, 1087.0),
    ];
    for (p, cx) in faders {
        g.add(
            p,
            Kind::Fader,
            rr(cx - 15.0, 236.0, 30.0, 122.0),
            Look::Fader,
            "",
        );
    }
    g.put(1155.0, 225.0, "LEFT", Align::Center, LABEL, TEXT);
    g.put(1204.0, 225.0, "RIGHT", Align::Center, LABEL, TEXT);

    // The bottom panels.
    mod_env_panel(g, 0, 43.0, 497.0);
    mod_env_panel(g, 1, 244.0, 497.0);
    lfo_panel(g, 0, 444.0, 497.0);
    lfo_panel(g, 1, 643.0, 497.0);
    misc_panel(g, 0, 843.0, 497.0);
    misc_panel(g, 1, 1043.0, 497.0);

    // The performance strip.
    g.put(52.0, 740.0, "BEND", Align::Left, LABEL, TEXT);
    g.put(52.0, 752.0, "RANGE", Align::Left, LABEL, TEXT);
    g.spin(ids::BEND, 50.0, 766.0, 42.0, 22.0, Look::Field(true));
    g.add(
        ids::WHEEL_PITCH,
        Kind::Wheel,
        rr(105.0, 722.0, 28.0, 103.0),
        Look::Wheel,
        "",
    );
    g.add(
        ids::WHEEL_MOD,
        Kind::Wheel,
        rr(143.0, 722.0, 28.0, 103.0),
        Look::Wheel,
        "",
    );
    keyboard(g);
    g.put(1160.0, 725.0, "MONO", Align::Center, LABEL, TEXT);
    g.put(1160.0, 737.0, "LEGATO", Align::Center, LABEL, TEXT);
    g.lamp(ids::MONO, Kind::Toggle, 1195.0, 718.0, 30.0, 23.0);
    g.put(1190.0, 757.0, "MODE", Align::Center, LABEL, TEXT);
    g.add(
        ids::PORTA_SCALE,
        Kind::Toggle,
        rr(1200.0, 766.0, 24.0, 37.0),
        Look::Slide,
        "",
    );
    g.put(1228.0, 775.0, "N", Align::Left, LABEL, TEXT);
    g.put(1228.0, 794.0, "S", Align::Left, LABEL, TEXT);
    g.knob(ids::PORTA, 1158.0, 779.0, 21.0, 36.0, "PORTAMENTO");
}

/// The keys: five octaves from C2, the white keys first and the black on top.
pub const KEY_FIRST: u8 = 36;
const KEY_X: f32 = 185.0;
const KEY_Y: f32 = 718.0;
const KEY_W: f32 = 923.0 / 35.0;
const KEY_H: f32 = 107.0;
const BLACK_W: f32 = 13.0;
const BLACK_H: f32 = 68.0;
const WHITE_STEPS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

fn keyboard(g: &mut Page) {
    for i in 0..35usize {
        let note = KEY_FIRST + 12 * (i / 7) as u8 + WHITE_STEPS[i % 7];
        let x = KEY_X + KEY_W * i as f32;
        g.add(
            0,
            Kind::Key(note),
            rr(x, KEY_Y, KEY_W, KEY_H),
            Look::Key(false),
            "",
        );
    }
    for i in 0..34usize {
        if matches!(i % 7, 2 | 6) {
            continue;
        }
        let note = KEY_FIRST + 12 * (i / 7) as u8 + WHITE_STEPS[i % 7] + 1;
        let x = KEY_X + KEY_W * (i + 1) as f32 - BLACK_W / 2.0;
        g.add(
            0,
            Kind::Key(note),
            rr(x, KEY_Y, BLACK_W, BLACK_H),
            Look::Key(true),
            "",
        );
    }
}

/// The display's contents.
fn display(g: &mut Page, ui: &Ui) {
    g.lcd = true;
    // The patch bar: four slots, the patch and its steppers.
    for i in 0..4 {
        g.nav(
            Nav::Tab(i),
            Look::Tab(["1", "2", "3", "4"][i]),
            ui.tab == i,
            404.0 + 24.0 * i as f32,
            225.0,
            23.0,
            24.0,
        );
    }
    g.nav(
        Nav::Patch(false),
        Look::Step(false),
        false,
        832.0,
        225.0,
        22.5,
        24.0,
    );
    g.nav(
        Nav::Patch(true),
        Look::Step(true),
        false,
        855.0,
        225.0,
        22.5,
        24.0,
    );
    g.nav(
        Nav::Menu,
        Look::Menu("MENU"),
        ui.menu,
        404.0,
        251.0,
        117.0,
        26.0,
    );

    // The effect list: a switch and a name to each.
    for (i, name) in FX_NAMES.iter().enumerate() {
        let y = 279.0 + 22.5 * i as f32;
        g.add(
            FX_ON[i],
            Kind::Toggle,
            rr(403.0, y + 1.0, 23.0, 21.0),
            Look::Check,
            "",
        );
        g.nav(
            Nav::Fx(i),
            Look::Tab(name),
            ui.fx == i,
            427.0,
            y,
            96.0,
            22.0,
        );
    }
    if ui.fx != 0 {
        for (i, &p) in FX_PARAMS[ui.fx].iter().enumerate() {
            let (col, row) = (i % 6, i / 6);
            g.cell(
                p,
                538.0 + 56.0 * col as f32,
                300.0 + 78.0 * row as f32,
                52.0,
                60.0,
            );
        }
        return;
    }
    // The arpeggiator.
    g.text(537.0, 302.0, "MODE");
    g.add(
        ids::ARP_MODE,
        Kind::Cycle,
        rr(606.0, 293.0, 90.0, 19.0),
        Look::Bar(false),
        "",
    );
    g.text(537.0, 325.0, "VELOCITY");
    g.add(
        ids::ARP_VELMODE,
        Kind::Cycle,
        rr(606.0, 316.0, 90.0, 19.0),
        Look::Bar(false),
        "",
    );
    g.text(537.0, 347.0, "OCTAVE");
    g.spin(ids::ARP_OCT, 606.0, 338.0, 90.0, 19.0, Look::Bar(true));
    g.text(537.0, 370.0, "WRAP");
    g.spin(ids::ARP_WRAP, 606.0, 361.0, 90.0, 19.0, Look::Bar(true));
    g.knob(ids::ARP_DIV, 746.0, 325.0, 26.0, 34.0, "TIME");
    g.knob(ids::ARP_GATE, 816.0, 325.0, 26.0, 34.0, "GATE");
    g.text(775.0, 384.0, "PAGE");
    g.nav(
        Nav::Page(1 - ui.page),
        Look::Menu("PAGE"),
        false,
        835.5,
        376.0,
        23.0,
        16.0,
    );
    g.text_r(607.0, 403.0, "HOLD");
    g.text_r(607.0, 420.0, "TRANSPOSE");
    g.text_r(607.0, 438.0, "VELOCITY");
    for i in 0..8 {
        let step = ui.page * 8 + i;
        let x = 618.0 + 29.75 * i as f32;
        g.add(
            ids::ARP_HOLD1 + step,
            Kind::Toggle,
            rr(x, 395.0, 29.75, 16.0),
            Look::Lamp,
            "",
        );
        g.add(
            ids::ARP_TR1 + step,
            Kind::Num,
            rr(x, 412.0, 29.75, 17.0),
            Look::Num,
            "",
        );
        g.add(
            ids::ARP_STEP1 + step,
            Kind::Num,
            rr(x, 430.0, 29.75, 17.0),
            Look::Num,
            "",
        );
    }
}

fn page(ui: &Ui) -> Page {
    let mut g = Page::default();
    front(&mut g, ui);
    display(&mut g, ui);
    g
}

/// The display's bezel and screen, in logical pixels.
fn display_rect() -> Rect {
    rr(360.0, 190.0, 555.0, 298.0)
}

fn screen_rect() -> Rect {
    rr(392.0, 214.0, 493.0, 254.0)
}

// ---- the drop-down lists ---------------------------------------------------------------------------------------

const ITEM_W: f32 = 104.0;

/// The rect of a list of `n` names opened from `anchor`, and of each name: in columns, below the anchor or above it,
/// inside the window.
pub fn popup_rects(anchor: Rect, n: usize) -> (Rect, Vec<Rect>) {
    let ih = if n <= 12 { 28.0 } else { 24.0 };
    let room = H as f32 - 12.0;
    let per_col = ((room / ih).floor() as usize).max(1);
    let cols = n.div_ceil(per_col).max(1);
    let rows = n.div_ceil(cols).max(1);
    let (w, h) = (cols as f32 * ITEM_W, rows as f32 * ih);
    let x = anchor.x.min(W as f32 - w - 4.0).max(4.0);
    let below = anchor.bottom() + 2.0;
    let y = if below + h <= H as f32 - 4.0 {
        below
    } else {
        (anchor.y - h - 2.0).max(4.0)
    };
    let items = (0..n)
        .map(|i| {
            Rect::new(
                x + (i / rows) as f32 * ITEM_W,
                y + (i % rows) as f32 * ih,
                ITEM_W,
                ih,
            )
        })
        .collect();
    (Rect::new(x, y, w, h), items)
}

/// The display's menu button.
fn menu_rect() -> Rect {
    rr(404.0, 251.0, 117.0, 26.0)
}

/// Every hit region of `ui`'s screen, the last on top (no painting). With a list or the menu open, only that and the
/// rest of the window that closes it.
pub fn layout(ui: &Ui) -> Vec<Hit> {
    let pg = page(ui);
    let dismiss = Hit {
        param: 0,
        kind: Kind::Dismiss,
        r: Rect::new(0.0, 0.0, W as f32, H as f32),
    };
    if ui.menu {
        let (_, rects) = popup_rects(menu_rect(), MENU_ITEMS.len());
        let mut hits = vec![dismiss];
        hits.extend(rects.into_iter().enumerate().map(|(i, r)| Hit {
            param: 0,
            kind: Kind::MenuItem(i),
            r,
        }));
        return hits;
    }
    if let Some(p) = ui.popup {
        let anchor = pg.items.iter().find(|i| i.param == p).map(|i| i.r);
        let mut hits = vec![dismiss];
        if let Some(anchor) = anchor {
            let (names, _) = choices(p);
            let (_, rects) = popup_rects(anchor, names.len());
            hits.extend(rects.into_iter().enumerate().map(|(i, r)| Hit {
                param: p,
                kind: Kind::Item(i),
                r,
            }));
        }
        return hits;
    }
    pg.items
        .iter()
        .map(|it| Hit {
            param: it.param,
            kind: it.kind,
            r: it.r,
        })
        .collect()
}

// ---- painting --------------------------------------------------------------------------------------------------

/// Paints the editor for `ui` with every control at its value in `v`, `pressed` the parameter being dragged and
/// `last` the one touched last (named on the display). Returns the hit regions, the last on top.
pub fn paint(
    c: &mut Canvas,
    v: &dyn Values,
    ui: &Ui,
    pressed: Option<usize>,
    last: Option<usize>,
) -> Vec<Hit> {
    c.begin_frame(None);
    c.clear(BG);
    paint_frame(c);
    let pg = page(ui);
    for p in &pg.panels {
        paint_panel(c, p);
    }
    paint_decor(c, v, ui);
    paint_display(c, v, ui, last);
    for t in &pg.texts {
        paint_text(c, t);
    }
    for it in &pg.items {
        paint_item(c, v, ui, it, pressed == Some(it.param));
    }
    if ui.menu {
        paint_menu(c);
    } else if let Some(p) = ui.popup
        && let Some(anchor) = pg.items.iter().find(|i| i.param == p).map(|i| i.r)
    {
        paint_popup(c, v, p, anchor);
    }
    layout(ui)
}

fn paint_text(c: &mut Canvas, t: &Text) {
    let w = c.text_width(t.font, t.s);
    let h = c.line_height(t.font);
    let x = match t.align {
        Align::Left => s(t.x),
        Align::Center => s(t.x) - w / 2.0,
        Align::Right => s(t.x) - w,
    };
    c.text(t.font, x.round(), (s(t.y) - h / 2.0).round(), t.color, t.s);
}

/// Text centred on (`x`, `y`) in design pixels.
fn mid(c: &mut Canvas, f: Font, x: f32, y: f32, color: Color, text: &str) {
    let r = Rect::new(s(x) - 60.0, s(y) - 8.0, 120.0, 16.0);
    c.text_center(f, r, color, text);
}

/// The wooden frame, the cream top bar and the dark strip under the panels.
fn paint_frame(c: &mut Canvas) {
    let (w, h) = (W as f32, H as f32);
    c.fill_rect(
        Rect::new(0.0, s(706.0), w, h - s(706.0)),
        Color::rgb(0x131313),
    );
    for (x0, x1) in [(0.0, s(37.0)), (s(1240.0), w)] {
        let r = Rect::new(x0, 0.0, x1 - x0, h);
        c.fill_rect(r, WOOD);
        // The grain: long lines at uneven distances, darker and lighter.
        let mut seed = 0x2f6b_u32 ^ (x0 as u32);
        let mut x = x0 + 1.0;
        while x < x1 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let dark = seed >> 31 == 0;
            let a = 0x18 + ((seed >> 20) & 0x1f) as u8;
            let col = if dark { WOOD_DARK } else { WOOD_LIGHT };
            let drift = ((seed >> 8) & 7) as f32 - 3.5;
            c.line(x, 0.0, x + drift, h, 1.1, col.with_alpha(a));
            x += 1.2 + ((seed >> 4) & 3) as f32;
        }
        c.fill_rect(Rect::new(x1 - 1.0, 0.0, 1.0, h), SHADOW);
        c.fill_rect(Rect::new(x0, 0.0, 1.0, h), SHADOW);
    }
    c.fill_rect(Rect::new(s(37.0), 0.0, s(1203.0), s(38.0)), BAR_BOTTOM);
    c.fill_rect_gradient(
        Rect::new(s(37.0), 0.0, s(1203.0), s(38.0)),
        BAR_TOP,
        BAR_BOTTOM,
        0.0,
    );
    c.fill_rect(
        Rect::new(s(37.0), s(38.0), s(1203.0), 1.0),
        Color::rgb(0x8f8579),
    );
    // The maker's mark.
    let (cx, cy) = (s(1040.0), s(19.0));
    c.stroke_circle(cx, cy, s(12.0), 1.6, BAR_TEXT);
    c.stroke_circle(cx, cy, s(6.0), 1.2, BAR_TEXT);
    c.line(cx - s(12.0), cy, cx + s(12.0), cy, 1.0, BAR_TEXT);
    c.text(
        Font::new(11).bold(),
        s(1062.0),
        cy - s(8.0),
        BAR_TEXT,
        "RIND AUDIO LABS",
    );
}

fn paint_panel(c: &mut Canvas, p: &Panel) {
    let r = p.r;
    let (lo, hi) = match p.skin {
        Skin::Brown => (BROWN, BROWN_HI),
        Skin::Graphite => (GRAPHITE, GRAPHITE_HI),
    };
    c.fill_rrect(r.inset(-1.0), 7.0, EDGE);
    c.fill_rect_gradient(r, hi, lo, 6.0);
    let head = Rect::new(r.x + s(3.0), r.y + s(3.0), r.w - s(6.0), s(19.0));
    c.fill_rrect(head, 6.0, HEAD);
    c.fill_rect(
        Rect::new(head.x + 4.0, head.y + 1.0, head.w - 8.0, 1.0),
        Color::rgb(0x3a2d22),
    );
    let f = TITLE;
    let w = c.text_width(f, &p.title);
    let lh = c.line_height(f);
    c.text(
        f,
        (head.cx() - w / 2.0).round(),
        (head.cy() - lh / 2.0).round(),
        HEAD_TEXT,
        &p.title,
    );
    if p.glyph {
        let (gx, gy) = (head.right() - s(15.0), head.cy());
        c.fill_circle(gx, gy, s(8.0), Color::rgb(0x6e625a));
        c.fill_circle(gx, gy - 0.4, s(7.0), Color::rgb(0xd9d1c6));
        c.polyline(
            &[(gx - 2.2, gy - 1.0), (gx, gy + 1.4), (gx + 2.2, gy - 1.0)],
            1.3,
            Color::rgb(0x2b231d),
        );
    }
}

/// The painted-only parts of the screen: scales, routing chips, the voice count, the meters and the name plate.
fn paint_decor(c: &mut Canvas, v: &dyn Values, ui: &Ui) {
    // Fader scales: 10, 5 and 0 either side of the amplitude and modulation envelopes and the mixer.
    let scales = |c: &mut Canvas, left: f32, right: f32, top: f32, bottom: f32| {
        for (i, label) in ["10", "5", "0"].into_iter().enumerate() {
            let y = top + (bottom - top) * i as f32 / 2.0;
            mid(c, SMALL, left, y, TEXT, label);
            if right > 0.0 {
                mid(c, SMALL, right, y, TEXT, label);
            }
        }
    };
    scales(c, 551.0, 724.0, 100.0, 176.0);
    for x in [43.0, 244.0] {
        scales(c, x + 9.0, x + 183.0, 572.0, 646.0);
    }
    scales(c, 948.0, 1056.0, 244.0, 350.0);
    scales(c, 1056.0, 0.0, 244.0, 350.0);
    scales(c, 1056.0, 0.0, 244.0, 350.0);
    // The filter's routing chips.
    let n = ui.part;
    let (a, b) = (
        v.plain(filt_param(n, ids::filt::IN_A)),
        v.plain(filt_param(n, ids::filt::IN_B)),
    );
    let from = match route_name(a, b) {
        "B" => "OSC B",
        "AB" => "OSC AB",
        "R" => "OSC R",
        _ => "OSC A",
    };
    let to = ["FILT A", "FILT B"][n];
    let dim = ["PILT A", "PILT B"][n];
    let chip = |c: &mut Canvas, r: Rect, on: bool, text: &str| {
        c.fill_rrect(r, 3.0, if on { CHIP } else { CHIP_DIM.with_alpha(0xa0) });
        let col = if on { CHIP_TEXT } else { Color::rgb(0x8b7f77) };
        c.text_center(LABEL, r, col, text);
    };
    chip(c, rr(60.0, 216.0, 52.0, 17.0), true, from);
    c.line(s(114.0), s(224.0), s(138.0), s(224.0), 1.0, TEXT);
    c.polyline(
        &[
            (s(133.0), s(221.0)),
            (s(138.0), s(224.0)),
            (s(133.0), s(227.0)),
        ],
        1.0,
        TEXT,
    );
    chip(c, rr(141.0, 216.0, 57.0, 17.0), true, to);
    chip(c, rr(60.0, 236.0, 52.0, 17.0), false, dim);
    // The voices sounding over the voices it may use.
    let field = rr(340.0, 9.0, 75.0, 21.0);
    c.fill_rrect(field, 3.0, FIELD);
    c.stroke_rrect(field, 3.0, 1.0, FIELD_EDGE);
    let text = format!("{} / {}", v.voices(), v.plain(ids::POLY).round() as i32);
    c.text_center(VALUE, field, FIELD_TEXT, &text);
    // The meters.
    paint_meters(c, v);
    // The name plate.
    let word = Font::new(58).bold().italic();
    c.text(
        word,
        s(996.0),
        s(378.0),
        Color::rgb(0x000000).with_alpha(0x90),
        "Rind",
    );
    c.text(word, s(990.0), s(372.0), Color::rgb(0xdadada), "Rind");
    let vw = c.text_width(Font::new(8), "v3.0.2");
    c.text(Font::new(8), s(1236.0) - vw, s(376.0), TEXT_DIM, "v3.0.2");
    c.text(
        Font::new(9),
        s(928.0),
        s(472.0),
        Color::rgb(0xc9c9c9),
        "\u{a9} 2024 Rind Audio Labs",
    );
}

fn paint_meters(c: &mut Canvas, v: &dyn Values) {
    let (l, r) = v.levels();
    let lit = [crate::shared::lamps(l), crate::shared::lamps(r)];
    const DB: [&str; 10] = [
        "+3", "0", "-3", "-6", "-8", "-12", "-15", "-18", "-21", "-24",
    ];
    for (row, label) in DB.iter().enumerate() {
        let y = 244.0 + 12.4 * row as f32;
        mid(c, SMALL, 1177.0, y, TEXT, label);
        for (side, x) in [1147.0, 1205.0].into_iter().enumerate() {
            let on = 9 - row < lit[side];
            let (col_on, col_off) = match row {
                0 => (LAMP_RED, LAMP_RED_OFF),
                1 | 2 => (LAMP_YELLOW, LAMP_YELLOW_OFF),
                _ => (LAMP_GREEN, LAMP_GREEN_OFF),
            };
            if on {
                c.fill_circle(s(x), s(y), s(8.0), col_on.with_alpha(0x40));
            }
            c.fill_circle(s(x), s(y), s(5.5), if on { col_on } else { col_off });
        }
        for x in [1161.0, 1194.0] {
            c.fill_rect(rr(x - 2.5, y - 2.5, 5.0, 5.0), Color::rgb(0xc9c9c9));
        }
    }
}

fn paint_display(c: &mut Canvas, v: &dyn Values, ui: &Ui, last: Option<usize>) {
    let d = display_rect();
    c.fill_rrect(d.inset(-1.0), 12.0, Color::rgb(0x0c0c0e));
    c.fill_rect_gradient(d, CASE_HI, CASE, 11.0);
    let sc = screen_rect();
    c.fill_rrect(sc.inset(-2.0), 5.0, Color::rgb(0x121214));
    c.fill_rrect(sc, 3.0, PAPER);
    // The patch bar.
    let bar = rr(500.0, 225.0, 334.0, 24.0);
    c.fill_rrect(bar, 2.0, BLUE);
    c.fill_circle(s(512.0), s(237.0), s(3.0), WHITE);
    let name = crate::presets::name(v.plain(ids::PATCH).round().max(0.0) as usize);
    let f = Font::new(10).bold();
    c.text_center(f, bar, WHITE, &name);
    // The line of the value last touched.
    let line = rr(524.0, 251.0, 350.0, 26.0);
    c.fill_rrect(line, 2.0, PAPER_HI);
    c.stroke_rrect(line, 2.0, 1.0, INK_DIM);
    let (label, value) = match last {
        Some(p) => (SPECS[p].name.to_owned(), v.text(p)),
        None => (
            "Rind".to_owned(),
            format!("{:.3}", v.norm(ids::MASTER_VOL) * 10.0),
        ),
    };
    let f = Font::new(10);
    let ty = (line.cy() - c.line_height(f) / 2.0).round();
    c.text(f, line.x + s(8.0), ty, INK, &label);
    c.text_center(f, line, INK, "=");
    c.text_right(f, line.right() - s(8.0), ty, INK, &value);
    // The panel the arpeggiator or an effect fills.
    let area = rr(525.0, 279.0, 347.0, 178.0);
    c.stroke_rrect(area, 2.0, 1.0, INK_DIM);
    c.fill_rrect(rr(850.0, 283.0, 20.0, 15.0), 2.0, BLUE);
    c.fill_polygon(
        &[
            (s(856.0), s(288.0)),
            (s(864.0), s(288.0)),
            (s(860.0), s(294.0)),
        ],
        WHITE,
    );
    if ui.fx == 0 {
        // The page box and the table's frame.
        let page = rr(813.0, 377.0, 15.0, 15.0);
        c.fill_rect(page, BLUE);
        c.text_center(VALUE, page, WHITE, &format!("{}", ui.page + 1));
        c.stroke_rrect(rr(618.0, 395.0, 238.0, 52.0), 0.0, 1.0, INK_DIM);
        for k in 1..8 {
            let x = s(618.0 + 29.75 * k as f32);
            c.line(x, s(395.0), x, s(447.0), 1.0, INK_DIM);
        }
        for y in [412.0, 430.0] {
            c.line(s(618.0), s(y), s(856.0), s(y), 1.0, INK_DIM);
        }
    } else {
        let text = FX_NAMES[ui.fx];
        c.text(VALUE, s(540.0), s(286.0), INK, text);
    }
}

fn paint_menu(c: &mut Canvas) {
    c.fill_rect(Rect::new(0.0, 0.0, W as f32, H as f32), SHADE);
    let (all, rects) = popup_rects(menu_rect(), MENU_ITEMS.len());
    c.fill_rrect(all.inset(-2.0), 5.0, INK_DIM);
    c.fill_rrect(all, 4.0, PAPER);
    for (r, name) in rects.iter().zip(MENU_ITEMS) {
        c.text_center(VALUE, *r, INK, name);
    }
}

/// The matrix slot of a target or amount parameter.
fn slot_of(p: usize) -> Option<usize> {
    let end = ids::MAT_BASE + 16 * ids::MAT_STRIDE;
    (p >= ids::MAT_BASE && p < end).then(|| (p - ids::MAT_BASE) / ids::MAT_STRIDE)
}

fn paint_item(c: &mut Canvas, v: &dyn Values, ui: &Ui, it: &Item, held: bool) {
    // A matrix row whose source is not the one its panel stands for and which is in use: veiled, with its source.
    let veiled = row_source(v, it.param)
        .filter(|&(sp, want)| (v.plain(sp) - want).abs() > 0.5)
        .filter(|_| {
            slot_of(it.param).is_some_and(|slot| {
                v.plain(mat_param(slot, ids::mat::DST)) > 0.5
                    || v.plain(mat_param(slot, ids::mat::AMT)).abs() > 1e-3
            })
        })
        .map(|(sp, _)| sp);
    let ctrl = ctrl_rect(it);
    match it.look {
        Look::Knob(rad) => paint_knob(c, v, it, rad, held),
        Look::Field(arrows) => paint_field(c, v, it, ctrl, arrows),
        Look::Icon => paint_icon(c, v, it.param, ctrl),
        Look::Bar(arrows) => paint_bar(c, v, it.param, ctrl, arrows, held),
        Look::Check => paint_check(c, v, it, ctrl),
        Look::Lamp => paint_lamp(c, v, it, ctrl),
        Look::Slide => paint_slide(c, v, it.param, ctrl),
        Look::Fader => paint_fader(c, v, it, held),
        Look::Num => paint_num(c, v, it.param, ctrl, held),
        Look::TopBtn(name) => paint_top_button(c, name, it.r, it.on),
        Look::Tab(name) => paint_tab(
            c,
            name,
            it.r,
            it.on,
            matches!(it.kind, Kind::Nav(Nav::Fx(_))),
        ),
        Look::Step(next) => paint_step(c, it.r, next),
        Look::Menu(name) => paint_menu_button(c, name, it.r, it.on),
        Look::Wheel => paint_wheel(c, v, it.param, it.r, held),
        Look::Key(black) => paint_key(
            c,
            it.r,
            black,
            matches!(it.kind, Kind::Key(n) if ui.keys >> n.min(127) & 1 == 1),
        ),
    }
    if !it.label.is_empty() && !matches!(it.look, Look::Knob(_)) {
        let text = it.label.to_uppercase();
        let f = c.fit_font(LABEL, &text, it.r.w);
        let w = c.text_width(f, &text);
        let col = if it.lcd { INK } else { TEXT };
        c.text(f, (it.r.cx() - w / 2.0).round(), it.r.y, col, &text);
    }
    if let Some(sp) = veiled {
        c.fill_rrect(ctrl, 3.0, VEIL);
        let names = ids::SOURCE_NAMES;
        let name = names[(v.plain(sp).round() as usize).min(names.len() - 1)];
        let f = c.fit_font(LABEL, name, ctrl.w - 4.0);
        c.text_center(f, ctrl, GOLD, name);
    }
}

/// The control inside an item's rect: under its label when it has one.
fn ctrl_rect(it: &Item) -> Rect {
    let r = it.r;
    if it.label.is_empty() {
        return r;
    }
    let top = s(16.0);
    let h = (r.h - top).min(s(26.0));
    Rect::new(r.x + 2.0, r.y + top, r.w - 4.0, h)
}

fn paint_knob(c: &mut Canvas, v: &dyn Values, it: &Item, rad: f32, held: bool) {
    let rad = s(rad);
    let n = v.norm(it.param).clamp(0.0, 1.0);
    let cx = it.r.cx();
    let cy = if it.label.is_empty() {
        it.r.cy()
    } else {
        it.r.y + s(4.0) + rad
    };
    let (start, sweep) = (0.75 * std::f32::consts::PI, 1.5 * std::f32::consts::PI);
    let tick = if it.lcd { INK } else { TICK };
    for i in 0..=10 {
        let a = start + sweep * i as f32 / 10.0;
        let (sa, ca) = a.sin_cos();
        let long = if i % 5 == 0 { 3.0 } else { 1.8 };
        c.line(
            cx + (rad + 2.0) * ca,
            cy + (rad + 2.0) * sa,
            cx + (rad + 2.0 + long) * ca,
            cy + (rad + 2.0 + long) * sa,
            0.8,
            tick,
        );
    }
    if rad >= s(14.0) {
        // The scale: 0 and 10 at the ends of the sweep, 5 at the top.
        mid_px(c, SMALL, cx, cy - rad - 8.5, tick, "5");
        mid_px(c, SMALL, cx - rad * 0.95, cy + rad + 4.5, tick, "0");
        mid_px(c, SMALL, cx + rad * 0.95, cy + rad + 4.5, tick, "10");
    }
    let a = start + sweep * n;
    let (sa, ca) = a.sin_cos();
    if it.lcd {
        c.fill_circle(cx + 0.8, cy + 1.2, rad, SHADOW);
        c.fill_circle(cx, cy, rad, if held { BLUE_DARK } else { BLUE });
        c.fill_circle(cx, cy, rad * 0.62, WHITE);
        c.fill_circle(cx, cy, rad * 0.5, if held { BLUE_DARK } else { BLUE });
        c.line(
            cx + rad * 0.25 * ca,
            cy + rad * 0.25 * sa,
            cx + rad * 0.98 * ca,
            cy + rad * 0.98 * sa,
            2.4,
            WHITE,
        );
    } else {
        c.fill_circle(cx + 0.8, cy + 1.6, rad + 1.0, SHADOW);
        c.fill_circle(cx, cy, rad + 0.6, SKIRT_HI);
        c.fill_circle(cx, cy, rad, SKIRT);
        // The knurled skirt: light notches round the rim.
        for i in 0..14 {
            let b = std::f32::consts::TAU * i as f32 / 14.0;
            let (sb, cb) = b.sin_cos();
            c.line(
                cx + rad * 0.82 * cb,
                cy + rad * 0.82 * sb,
                cx + rad * 0.97 * cb,
                cy + rad * 0.97 * sb,
                0.7,
                SKIRT_HI,
            );
        }
        let cap = rad * 0.7;
        c.fill_circle(cx, cy, cap, CAP_LO);
        c.fill_circle(cx - cap * 0.12, cy - cap * 0.14, cap * 0.86, CAP_MID);
        c.fill_circle(
            cx - cap * 0.2,
            cy - cap * 0.26,
            cap * 0.5,
            CAP_HI.with_alpha(0xb0),
        );
        if held {
            c.stroke_circle(cx, cy, rad + 1.5, 1.2, GOLD);
        }
        c.line(
            cx + rad * 0.2 * ca,
            cy + rad * 0.2 * sa,
            cx + rad * 0.98 * ca,
            cy + rad * 0.98 * sa,
            2.2,
            Color::rgb(0x1c1c1c),
        );
        c.line(
            cx + rad * 0.55 * ca,
            cy + rad * 0.55 * sa,
            cx + rad * 0.96 * ca,
            cy + rad * 0.96 * sa,
            1.0,
            CAP_HI,
        );
    }
    if !it.label.is_empty() {
        let text = it.label.to_uppercase();
        let f = c.fit_font(LABEL, &text, it.r.w + 8.0);
        let w = c.text_width(f, &text);
        let col = if it.lcd { INK } else { TEXT };
        c.text(
            f,
            (cx - w / 2.0).round(),
            (cy + rad + s(16.0)).round(),
            col,
            &text,
        );
    }
}

/// Text centred on a point in logical pixels.
fn mid_px(c: &mut Canvas, f: Font, x: f32, y: f32, color: Color, text: &str) {
    c.text_center(f, Rect::new(x - 20.0, y - 6.0, 40.0, 12.0), color, text);
}

fn arrows(c: &mut Canvas, zone: Rect, color: Color) {
    let (cx, cy) = (zone.cx(), zone.cy());
    let a = (zone.w * 0.28).clamp(2.0, 3.5);
    c.fill_polygon(
        &[
            (cx - a, cy - 1.2),
            (cx + a, cy - 1.2),
            (cx, cy - 1.2 - a * 1.5),
        ],
        color,
    );
    c.fill_polygon(
        &[
            (cx - a, cy + 1.2),
            (cx + a, cy + 1.2),
            (cx, cy + 1.2 + a * 1.5),
        ],
        color,
    );
}

/// The text a spin field shows: a whole number plainly, a name as it is.
fn field_text(v: &dyn Values, p: usize) -> String {
    match SPECS[p].fmt {
        Fmt::Int | Fmt::Oct | Fmt::Semi => format!("{}", v.plain(p).round() as i32),
        _ => v.text(p),
    }
}

fn paint_field(c: &mut Canvas, v: &dyn Values, it: &Item, r: Rect, arrows_beside: bool) {
    let zone = if arrows_beside { s(15.0) } else { 0.0 };
    let b = Rect::new(r.x, r.y, r.w - zone, r.h);
    c.fill_rrect(b, 3.0, FIELD);
    c.stroke_rrect(b, 3.0, 1.0, FIELD_EDGE);
    let text = if it.kind == Kind::Route {
        let (a, bb) = (it.param, it.param + 1);
        route_name(v.plain(a), v.plain(bb)).to_owned()
    } else if it.kind == Kind::Cycle {
        let t = v.text(it.param);
        if t == "Off" { "-".to_owned() } else { t }
    } else {
        field_text(v, it.param)
    };
    let f = c.fit_font(VALUE.italic(), &text, b.w - 4.0);
    c.text_center(f, b, FIELD_TEXT, &text);
    if arrows_beside {
        arrows(c, Rect::new(b.right(), r.y, zone, r.h), ARROW);
    }
}

fn paint_icon(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect) {
    let zone = s(15.0);
    let b = Rect::new(r.x, r.y, r.w - zone, r.h);
    c.fill_rrect(b, 4.0, Color::rgb(0x1a1a1a));
    c.stroke_rrect(b, 4.0, 1.0, Color::rgb(0x4a4a4a));
    let icon = Rect::new(b.x + 4.0, b.y + 3.0, b.w - 8.0, b.h - 6.0);
    let now = v.plain(p).round() as i32;
    if is_wave(p) {
        wave_icon(c, now, icon);
    } else if is_lfo_wave(p) {
        shape_icon(c, icon, |t| lfo_shape(now, t));
    } else {
        // A filter's response: low-pass, band-pass or high-pass.
        shape_icon(c, icon, |t| match now {
            0 => {
                if t < 0.55 {
                    0.7
                } else {
                    0.7 - (t - 0.55) * 3.4
                }
            }
            1 => 0.75 - ((t - 0.5) * 2.0).powi(2) * 1.5,
            _ => {
                if t > 0.45 {
                    0.7
                } else {
                    0.7 - (0.45 - t) * 3.4
                }
            }
        });
    }
    arrows(c, Rect::new(b.right(), r.y, zone, r.h), ARROW);
}

fn shape_icon(c: &mut Canvas, r: Rect, f: impl Fn(f32) -> f32) {
    let pts: Vec<(f32, f32)> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            (r.x + t * r.w, r.cy() - f(t).clamp(-1.0, 1.0) * r.h * 0.5)
        })
        .collect();
    c.polyline(&pts, 1.5, CHIP);
}

fn paint_bar(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, spin: bool, held: bool) {
    c.fill_rrect(r, 2.0, if held { BLUE_DARK } else { BLUE });
    let text = field_text(v, p);
    let body = Rect::new(r.x, r.y, r.w - if spin { s(14.0) } else { 0.0 }, r.h);
    let f = c.fit_font(VALUE, &text, body.w - 4.0);
    c.text_center(f, body, WHITE, &text);
    if spin {
        arrows(c, Rect::new(r.right() - s(14.0), r.y, s(12.0), r.h), WHITE);
    }
}

fn paint_check(c: &mut Canvas, v: &dyn Values, it: &Item, r: Rect) {
    let on = v.plain(it.param) > 0.5;
    if it.lcd {
        c.fill_rrect(r, 2.0, PAPER_HI);
        c.stroke_rrect(r, 2.0, 1.0, INK_DIM);
        if on {
            c.polyline(
                &[
                    (r.x + r.w * 0.22, r.cy()),
                    (r.x + r.w * 0.44, r.bottom() - r.h * 0.26),
                    (r.right() - r.w * 0.2, r.y + r.h * 0.26),
                ],
                2.0,
                BLUE_DARK,
            );
        }
    } else {
        c.fill_rrect(r, 3.0, Color::rgb(0x9a9a9a));
        c.stroke_rrect(r, 3.0, 1.0, EDGE);
        if on {
            c.polyline(
                &[
                    (r.x + r.w * 0.22, r.cy()),
                    (r.x + r.w * 0.44, r.bottom() - r.h * 0.26),
                    (r.right() - r.w * 0.2, r.y + r.h * 0.26),
                ],
                2.0,
                EDGE,
            );
        }
    }
}

fn paint_lamp(c: &mut Canvas, v: &dyn Values, it: &Item, r: Rect) {
    let low = v.plain(it.param) <= 0.5;
    let on = match it.kind {
        Kind::Switch(true) => low,
        _ => !low,
    };
    if it.lcd {
        // A cell of the table: a tie bar when on.
        c.fill_rect(r.inset(0.5), if on { BLUE } else { PAPER });
        if on {
            let (y, a) = (r.cy(), r.w * 0.22);
            c.line(r.cx() - a, y, r.cx() + a, y, 1.6, WHITE);
            c.line(r.cx() - a, y - 3.0, r.cx() - a, y + 3.0, 1.6, WHITE);
            c.line(r.cx() + a, y - 3.0, r.cx() + a, y + 3.0, 1.6, WHITE);
        }
        return;
    }
    if on {
        c.fill_rrect(r.inset(-2.0), 6.0, LAMP_GLOW);
    }
    c.fill_rrect(r, 4.0, if on { LAMP_ON } else { LAMP_OFF });
    c.stroke_rrect(
        r,
        4.0,
        1.0,
        if on {
            Color::rgb(0xffffff)
        } else {
            Color::rgb(0x4a4a4a)
        },
    );
}

fn paint_slide(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect) {
    let n = v.plain(p) > 0.5;
    let slot = Rect::new(r.cx() - r.w * 0.28, r.y, r.w * 0.56, r.h);
    c.fill_rrect(slot, slot.w / 2.0, SLOT);
    c.stroke_rrect(slot, slot.w / 2.0, 1.0, Color::rgb(0x3c3c3c));
    let y = if n {
        slot.bottom() - slot.w
    } else {
        slot.y + slot.w * 0.4
    };
    c.fill_circle(slot.cx(), y + slot.w / 2.0 - 0.6, slot.w * 0.75, SHADOW);
    c.fill_circle(slot.cx(), y + slot.w / 2.0 - 1.0, slot.w * 0.7, CAP_MID);
    c.fill_circle(
        slot.cx() - 1.0,
        y + slot.w / 2.0 - 2.0,
        slot.w * 0.3,
        CAP_HI,
    );
}

fn paint_fader(c: &mut Canvas, v: &dyn Values, it: &Item, held: bool) {
    let n = v.norm(it.param).clamp(0.0, 1.0);
    let r = it.r;
    let cx = r.cx();
    let (top, bot) = (r.y + FADER_PAD, r.bottom() - FADER_PAD);
    for i in 0..=10 {
        let y = top + (bot - top) * i as f32 / 10.0;
        let long = if i % 5 == 0 { 6.0 } else { 3.5 };
        c.line(cx - 12.0, y, cx - 12.0 + long, y, 0.9, TICK);
        c.line(cx + 12.0, y, cx + 12.0 - long, y, 0.9, TICK);
    }
    c.fill_rrect(
        Rect::new(cx - 2.5, top - 2.0, 5.0, bot - top + 4.0),
        2.5,
        SLOT,
    );
    let y = top + (1.0 - n) * (bot - top);
    let cap = Rect::new(cx - 9.0, y - 5.5, 18.0, 11.0);
    c.fill_rrect(Rect::new(cx - 9.5, y - 4.0, 19.0, 11.5), 3.0, SHADOW);
    c.fill_rect_gradient(
        cap,
        if held { GOLD } else { Color::rgb(0xd0d0d0) },
        Color::rgb(0x55555a),
        3.0,
    );
    c.fill_rect(
        Rect::new(cx - 8.0, y - 0.6, 16.0, 1.2),
        Color::rgb(0x1a1a1a),
    );
    c.fill_rect(
        Rect::new(cx - 8.0, y + 0.6, 16.0, 0.8),
        Color::rgb(0xf0f0f0),
    );
}

fn num_text(v: &dyn Values, p: usize) -> String {
    if step_of(p).is_some() && (ids::ARP_STEP1..ids::ARP_STEP1 + 16).contains(&p) {
        format!("{}", (v.plain(p) * 127.0).round() as i32)
    } else if is_int(p) {
        match v.plain(p).round() as i32 {
            0 => "0".to_owned(),
            n => format!("{n:+}"),
        }
    } else {
        format!("{}", (v.plain(p) * 100.0).round() as i32)
    }
}

fn paint_num(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, held: bool) {
    let live = step_of(p).is_none_or(|st| (st as f32) < v.plain(ids::ARP_STEPS));
    c.fill_rect(r.inset(0.5), if held { PAPER_HI } else { PAPER });
    c.text_center(VALUE, r, if live { INK } else { INK_DIM }, &num_text(v, p));
}

fn paint_top_button(c: &mut Canvas, name: &str, r: Rect, on: bool) {
    c.fill_rrect(r, 3.0, if on { LAMP_ON } else { Color::rgb(0xb9b2a6) });
    c.stroke_rrect(
        r,
        3.0,
        1.0,
        if on {
            Color::rgb(0xc99a63)
        } else {
            Color::rgb(0x8f877b)
        },
    );
    c.text_center(
        VALUE,
        r,
        if on {
            Color::rgb(0xb8651f)
        } else {
            Color::rgb(0x7a7268)
        },
        name,
    );
}

fn paint_tab(c: &mut Canvas, name: &str, r: Rect, on: bool, listed: bool) {
    c.fill_rect(r, if on { PAPER_HI } else { BLUE });
    if on {
        c.stroke_rrect(r, 0.0, 1.0, INK_DIM);
    }
    let f = if listed {
        Font::new(9).bold().italic()
    } else {
        VALUE
    };
    let f = c.fit_font(f, name, r.w - 4.0);
    c.text_center(f, r, if on { INK } else { WHITE }, name);
}

fn paint_step(c: &mut Canvas, r: Rect, next: bool) {
    c.fill_rect(r, BLUE_DARK);
    let (cx, cy, a) = (r.cx(), r.cy(), r.h * 0.22);
    let d = if next { 1.0 } else { -1.0 };
    c.fill_polygon(
        &[
            (cx - d * a, cy - a * 1.3),
            (cx - d * a, cy + a * 1.3),
            (cx + d * a, cy),
        ],
        WHITE,
    );
}

fn paint_menu_button(c: &mut Canvas, name: &str, r: Rect, on: bool) {
    if name == "PAGE" {
        c.fill_rect(r, BLUE);
        let (cx, cy, a) = (r.cx(), r.cy(), r.h * 0.22);
        c.fill_polygon(
            &[
                (cx - a * 1.4, cy - a),
                (cx + a * 1.4, cy - a),
                (cx, cy + a * 1.2),
            ],
            WHITE,
        );
        return;
    }
    c.fill_rect(r, if on { BLUE_DARK } else { BLUE });
    let f = Font::new(10).bold();
    let ty = (r.cy() - c.line_height(f) / 2.0).round();
    c.text(f, r.x + s(8.0), ty, WHITE, name);
    arrows(c, Rect::new(r.right() - s(18.0), r.y, s(14.0), r.h), WHITE);
}

fn paint_wheel(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, held: bool) {
    let n = v.norm(p).clamp(0.0, 1.0);
    c.fill_rrect(r.inset(-1.0), 5.0, Color::rgb(0x050505));
    c.fill_rect_gradient(r, Color::rgb(0x4a4a4a), Color::rgb(0x0e0e0e), 4.0);
    // The ridges roll with the value.
    let step = 6.0;
    let off = n * 60.0 % step;
    let mut y = r.y + off;
    while y < r.bottom() {
        c.line(
            r.x + 2.0,
            y,
            r.right() - 2.0,
            y,
            1.0,
            Color::rgb(0x8a8a8a).with_alpha(0x70),
        );
        y += step;
    }
    c.fill_rect(
        Rect::new(r.cx() - 1.0, r.y + 2.0, 2.0, r.h - 4.0),
        Color::rgb(0xffffff).with_alpha(if held { 0x50 } else { 0x28 }),
    );
}

fn paint_key(c: &mut Canvas, r: Rect, black: bool, down: bool) {
    if black {
        c.fill_rect_gradient(
            r,
            if down { KEY_DOWN } else { Color::rgb(0x3a3a3a) },
            KEY_BLACK,
            1.5,
        );
        c.fill_rect(
            Rect::new(r.x + 1.0, r.y, 1.0, r.h * 0.85),
            Color::rgb(0xffffff).with_alpha(0x28),
        );
    } else {
        c.fill_rect_gradient(
            r.inset(0.4),
            if down { KEY_DOWN } else { KEY_WHITE },
            if down { KEY_DOWN } else { KEY_SHADE },
            1.0,
        );
        c.fill_rect(
            Rect::new(r.right() - 0.8, r.y, 0.8, r.h),
            Color::rgb(0x6a6a6a),
        );
    }
}

fn paint_popup(c: &mut Canvas, v: &dyn Values, p: usize, anchor: Rect) {
    c.fill_rect(Rect::new(0.0, 0.0, W as f32, H as f32), SHADE);
    let (names, lo) = choices(p);
    let (all, rects) = popup_rects(anchor, names.len());
    c.fill_rrect(all.inset(-2.0), 5.0, INK_DIM);
    c.fill_rrect(all, 4.0, PAPER);
    let now = v.plain(p).round() as i32 - lo;
    for (i, (r, name)) in rects.iter().zip(names).enumerate() {
        let on = i as i32 == now;
        if on {
            c.fill_rrect(r.inset(1.5), 3.0, BLUE);
        }
        let f = c.fit_font(VALUE, name, r.w - 10.0);
        c.text_center(f, *r, if on { WHITE } else { INK }, name);
    }
}

fn is_wave(p: usize) -> bool {
    let end = ids::OSC_BASE + 4 * ids::OSC_STRIDE;
    p >= ids::OSC_BASE && p < end && (p - ids::OSC_BASE) % ids::OSC_STRIDE == ids::osc::WAVE
}

fn is_lfo_wave(p: usize) -> bool {
    p == lfo_param(0, ids::lfo::WAVE) || p == lfo_param(1, ids::lfo::WAVE)
}

/// The step an arpeggiator per-step parameter belongs to (0-based).
fn step_of(p: usize) -> Option<usize> {
    [ids::ARP_STEP1, ids::ARP_TR1, ids::ARP_HOLD1]
        .iter()
        .find(|&&b| p >= b && p < b + 16)
        .map(|&b| p - b)
}

// ---- labels ----------------------------------------------------------------------------------------------------

/// The prefixes the table's names share within a group, stripped from a label.
const PREFIXES: [&str; 20] = [
    "Osc A1 ",
    "Osc A2 ",
    "Osc B1 ",
    "Osc B2 ",
    "Part A ",
    "Part B ",
    "Filter 1 ",
    "Filter 2 ",
    "Env 3 ",
    "Env 4 ",
    "LFO 1 ",
    "LFO 2 ",
    "Distortion ",
    "Phaser ",
    "Chorus ",
    "EQ ",
    "Delay ",
    "Reverb ",
    "Compressor ",
    "Arp ",
];

/// What a name becomes where a label is a few letters wide.
const SHORT: [(&str, &str); 43] = [
    ("Global Filter Cutoff", "Cutoff"),
    ("Global Filter Resonance", "Resonance"),
    ("Global Filter Key Track", "Key Track"),
    ("Pitch Wheel", "Pitch"),
    ("Master Volume", "Master"),
    ("Warm Drive", "Warm"),
    ("Master Tune", "Tune"),
    ("Polyphony", "Poly"),
    ("Voice Mode", "Mode"),
    ("Portamento", "Porta"),
    ("Portamento Mode", "Glide"),
    ("Bend Range", "Bend"),
    ("Velocity Sensitivity", "Vel"),
    ("Attack", "Att"),
    ("Decay", "Dec"),
    ("Sustain", "Sus"),
    ("Release", "Rel"),
    ("Octave", "Oct"),
    ("Note", "Semi"),
    ("Volume", "Vol"),
    ("Phase Random", "Rand"),
    ("Invert", "Inv"),
    ("Resonance", "Reso"),
    ("Key Track", "Key"),
    ("Input A", "In A"),
    ("Input B", "In B"),
    ("Division", "Div"),
    ("Offset", "Offs"),
    ("Retrigger", "Trig"),
    ("Velocity Source", "Vel"),
    ("Frequency", "Freq"),
    ("Feedback", "Fdbk"),
    ("Bass Frequency", "Bass F"),
    ("Bass Gain", "Bass G"),
    ("Treble Frequency", "Treb F"),
    ("Treble Gain", "Treb G"),
    ("Division L", "Div L"),
    ("Division R", "Div R"),
    ("Ping-Pong", "Ping"),
    ("Low-pass", "LP"),
    ("High-pass", "HP"),
    ("Pre-delay", "Pre"),
    ("Threshold", "Thresh"),
];

/// Parameter `p`'s label over its control.
pub fn short(p: usize) -> &'static str {
    let name = SPECS[p].name;
    let name = PREFIXES
        .iter()
        .find_map(|pre| name.strip_prefix(pre))
        .unwrap_or(name);
    SHORT
        .iter()
        .find(|(long, _)| *long == name)
        .map_or(name, |(_, s)| *s)
}

fn wave_icon(c: &mut Canvas, wave: i32, r: Rect) {
    let pts: Vec<(f32, f32)> = (0..=18)
        .map(|i| {
            let t = i as f32 / 18.0;
            (r.x + t * r.w, r.cy() - wave_shape(wave, t) * r.h * 0.45)
        })
        .collect();
    c.polyline(&pts, 1.5, CHIP);
}

/// One cycle of oscillator wave `wave` (see [`ids::WAVE_NAMES`]) at phase `t` in `[0, 1]`, for the icons.
fn wave_shape(wave: i32, t: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let tri = 1.0 - 4.0 * ((t + 0.25).rem_euclid(1.0) - 0.5).abs();
    let pulse = |width: f32| if t < width { 1.0 } else { -1.0 };
    match wave {
        0 => (t * tau).sin(),
        1 => 1.0 - 2.0 * t,
        2 => tri,
        3 => pulse(0.5),
        4 => pulse(0.25),
        5 => pulse(0.125),
        6 => 0.5 * (1.0 - 2.0 * t) + 0.5 * tri,
        _ => (((t * 7919.0).sin() * 43758.5).fract() * 2.0 - 1.0).clamp(-1.0, 1.0),
    }
}

/// One cycle of LFO wave `wave` (see [`ids::LFO_WAVE_NAMES`]) at phase `t` in `[0, 1]`, for the icons.
fn lfo_shape(wave: i32, t: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let tri = 1.0 - 4.0 * ((t + 0.25).rem_euclid(1.0) - 0.5).abs();
    let pulse = |width: f32| if t < width { 1.0 } else { -1.0 };
    match wave {
        0 => (t * tau).sin(),
        1 => tri,
        2 => 2.0 * t - 1.0,
        3 => 1.0 - 2.0 * t,
        4 => pulse(0.5),
        5 => pulse(0.25),
        6 => pulse(0.125),
        7 => [0.6, -0.3, 0.9, -0.8, 0.2][((t * 5.0) as usize).min(4)],
        8 => (t * tau).sin() * 0.6 + (t * tau * 2.3).sin() * 0.4,
        9 => ((t * 4.0).floor() / 3.0) * 2.0 - 1.0,
        _ => (t * tau).sin().abs() * 2.0 - 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every screen: each part, with the display on each effect and each arpeggiator page.
    fn screens() -> Vec<Ui> {
        let mut out = Vec::new();
        for part in 0..2 {
            for fx in 0..FX_NAMES.len() {
                for page in 0..if fx == 0 { 2 } else { 1 } {
                    out.push(Ui {
                        part,
                        fx,
                        page,
                        ..Ui::default()
                    });
                }
            }
        }
        out
    }

    fn window() -> Rect {
        Rect::new(0.0, 0.0, W as f32, H as f32)
    }

    fn inside(win: Rect, r: Rect) -> bool {
        let i = win.intersect(&r);
        (i.w - r.w).abs() < 0.01 && (i.h - r.h).abs() < 0.01
    }

    fn apart(a: Rect, b: Rect) -> bool {
        let i = a.intersect(&b);
        i.w < 0.01 || i.h < 0.01
    }

    fn key(h: &Hit) -> bool {
        matches!(h.kind, Kind::Key(_))
    }

    #[test]
    fn every_screen_lays_out_inside_the_window_without_overlap() {
        for ui in screens() {
            let hits = layout(&ui);
            for (i, a) in hits.iter().enumerate() {
                assert!(inside(window(), a.r), "{ui:?}: {a:?} leaves the window");
                for b in &hits[i + 1..] {
                    // The black keys stand on the white ones.
                    if key(a) && key(b) {
                        continue;
                    }
                    assert!(apart(a.r, b.r), "{ui:?}: {a:?} overlaps {b:?}");
                }
            }
        }
    }

    #[test]
    fn controls_are_big_enough_to_touch() {
        for ui in screens() {
            for h in layout(&ui) {
                let (w, hh) = match h.kind {
                    Kind::Fader => (14.0, 50.0),
                    Kind::Key(n) if matches!(n % 12, 1 | 3 | 6 | 8 | 10) => (7.0, 30.0),
                    Kind::Key(_) => (14.0, 60.0),
                    _ => (14.0, 10.0),
                };
                assert!(h.r.w >= w && h.r.h >= hh, "{ui:?}: {h:?}");
            }
        }
    }

    /// The parameters the design draws no control for: they stay in the table (a song or a host reaches them) and the
    /// sound, but the screen shows only what the design shows.
    const UNDRAWN: [&str; 26] = [
        "master_tune",
        "porta_mode",
        "vel_amp",
        "pa_on",
        "pa_pan",
        "pb_on",
        "pb_pan",
        "f1_on",
        "f1_in_b",
        "f1_vol",
        "f1_pan",
        "f2_on",
        "f2_in_b",
        "f2_vol",
        "f2_pan",
        "l1_sync",
        "l1_div",
        "l1_phase",
        "l2_sync",
        "l2_div",
        "l2_phase",
        "arp_hold",
        "arp_steps",
        "f1_key",
        "f2_key",
        "patch",
    ];

    #[test]
    fn every_parameter_has_a_control_unless_the_design_leaves_it_out() {
        let mut seen = vec![false; ids::COUNT];
        for ui in screens() {
            for h in layout(&ui) {
                if h.is_param() {
                    seen[h.param] = true;
                }
            }
        }
        // A source the screen sets for its rows has no control of its own; a filter's second input rides on its first
        // (the input select).
        let missing: Vec<_> = (0..ids::COUNT)
            .filter(|&i| !seen[i] && !derived_source(i))
            .map(|i| SPECS[i].id)
            .filter(|id| !UNDRAWN.contains(id))
            .collect();
        assert!(missing.is_empty(), "no control for {missing:?}");
        for id in UNDRAWN {
            let i = SPECS.iter().position(|s| s.id == id).expect(id);
            assert!(!seen[i], "{id} has a control after all");
        }
    }

    #[test]
    fn no_parameter_has_two_controls_on_one_screen_but_the_filter_control_mirrors() {
        for ui in screens() {
            let mut seen = std::collections::HashSet::new();
            let mirrored = [
                filt_param(ui.part, ids::filt::CUTOFF),
                filt_param(ui.part, ids::filt::RES),
            ];
            for h in layout(&ui).iter().filter(|h| h.is_param()) {
                if mirrored.contains(&h.param) {
                    continue;
                }
                assert!(
                    seen.insert(h.param),
                    "{ui:?}: {} is twice",
                    SPECS[h.param].id
                );
            }
        }
    }

    #[test]
    fn every_drop_down_list_fits_the_window_and_holds_every_name() {
        for ui in screens() {
            for h in layout(&ui)
                .iter()
                .filter(|h| h.kind == Kind::Cycle && dropdown(h.param))
            {
                let open = Ui {
                    popup: Some(h.param),
                    ..ui
                };
                let hits = layout(&open);
                let items: Vec<_> = hits
                    .iter()
                    .filter(|i| matches!(i.kind, Kind::Item(_)))
                    .collect();
                let (names, _) = choices(h.param);
                assert_eq!(items.len(), names.len(), "{}", SPECS[h.param].id);
                for (k, a) in items.iter().enumerate() {
                    assert!(inside(window(), a.r), "{}: {a:?}", SPECS[h.param].id);
                    assert!(a.r.w >= 90.0 && a.r.h >= 22.0, "{a:?}");
                    for b in &items[k + 1..] {
                        assert!(apart(a.r, b.r), "{a:?} overlaps {b:?}");
                    }
                }
                assert_eq!(hits[0].kind, Kind::Dismiss);
            }
        }
    }

    #[test]
    fn the_menu_lists_its_entries_inside_the_window() {
        let open = Ui {
            menu: true,
            ..Ui::default()
        };
        let hits = layout(&open);
        assert_eq!(hits[0].kind, Kind::Dismiss);
        let items: Vec<_> = hits
            .iter()
            .filter(|h| matches!(h.kind, Kind::MenuItem(_)))
            .collect();
        assert_eq!(items.len(), MENU_ITEMS.len());
        for a in items {
            assert!(inside(window(), a.r) && a.r.h >= 22.0, "{a:?}");
        }
    }

    #[test]
    fn selectors_with_many_names_are_drop_downs_and_boxes_with_arrows_step() {
        assert!(dropdown(mat_param(8, ids::mat::SRC)));
        assert!(dropdown(mat_param(0, ids::mat::DST)));
        assert!(dropdown(ids::ARP_MODE));
        assert!(dropdown(ids::ARP_WRAP));
        assert!(!dropdown(ids::MONO));
        assert!(!dropdown(filt_param(0, ids::filt::SLOPE)));
        assert!(!dropdown(filt_param(0, ids::filt::TYPE)));
        assert!(!dropdown(ids::MASTER_VOL));
        // What the design draws with arrows steps; no list.
        let hits = layout(&Ui::default());
        for p in [
            osc_param(0, ids::osc::WAVE),
            ids::ARP_WRAP,
            ids::POLY,
            filt_param(0, ids::filt::TYPE),
            lfo_param(0, ids::lfo::WAVE),
        ] {
            let h = hits.iter().find(|h| h.param == p).expect(SPECS[p].id);
            assert_eq!(h.kind, Kind::Spin, "{}", SPECS[p].id);
        }
    }

    #[test]
    fn the_matrix_rows_cover_the_sixteen_slots() {
        // Every slot has a target and an amount control on the screen (the part never matters to them).
        let hits = layout(&Ui::default());
        for slot in 0..16 {
            for f in [ids::mat::DST, ids::mat::AMT] {
                assert!(
                    hits.iter().any(|h| h.param == mat_param(slot, f)),
                    "slot {slot} field {f}"
                );
            }
            let src = mat_param(slot, ids::mat::SRC);
            let has = hits.iter().any(|h| h.param == src);
            assert_eq!(has, slot_src(slot) == SlotSrc::Free, "slot {slot}");
        }
    }

    /// The defaults, as the editor would show them.
    struct Defaults;

    impl Values for Defaults {
        fn norm(&self, p: usize) -> f32 {
            crate::ranges::to_norm(p, SPECS[p].default)
        }
        fn plain(&self, p: usize) -> f32 {
            SPECS[p].default
        }
        fn text(&self, p: usize) -> String {
            format!("{}", SPECS[p].default)
        }
        fn levels(&self) -> (f32, f32) {
            (0.7, 0.4)
        }
        fn voices(&self) -> usize {
            2
        }
    }

    #[test]
    fn a_rows_source_follows_its_panel_or_its_groups_first_slot() {
        let v = Defaults;
        assert_eq!(
            row_source(&v, mat_param(0, ids::mat::DST)),
            Some((mat_param(0, ids::mat::SRC), ids::msrc::ENV_3 as f32))
        );
        assert_eq!(
            row_source(&v, mat_param(7, ids::mat::AMT)),
            Some((mat_param(7, ids::mat::SRC), ids::msrc::LFO_2 as f32))
        );
        assert_eq!(row_source(&v, mat_param(8, ids::mat::DST)), None);
        assert_eq!(
            row_source(&v, mat_param(9, ids::mat::AMT)),
            Some((mat_param(9, ids::mat::SRC), 0.0))
        );
        assert_eq!(row_source(&v, ids::MASTER_VOL), None);
        // A group's first source drags its second along.
        assert_eq!(
            partner(mat_param(8, ids::mat::SRC)),
            Some(mat_param(9, ids::mat::SRC))
        );
        assert_eq!(partner(mat_param(9, ids::mat::SRC)), None);
        assert_eq!(partner(mat_param(0, ids::mat::SRC)), None);
        assert_eq!(partner(mat_param(8, ids::mat::DST)), None);
        assert!(derived_source(mat_param(3, ids::mat::SRC)));
        assert!(derived_source(mat_param(9, ids::mat::SRC)));
        assert!(!derived_source(mat_param(8, ids::mat::SRC)));
    }

    #[test]
    fn widgets_follow_the_spec() {
        assert_eq!(widget(ids::ARP_ON), Kind::Toggle);
        assert_eq!(widget(ids::WARM), Kind::Toggle);
        assert_eq!(widget(ids::ARP_MODE), Kind::Cycle);
        assert_eq!(widget(ids::MASTER_VOL), Kind::Knob);
        assert!(is_int(ids::POLY));
        assert!(bipolar(ids::MASTER_TUNE));
        assert!(!bipolar(ids::MASTER_VOL));
    }

    #[test]
    fn labels_are_short() {
        for p in (0..ids::COUNT)
            .filter(|&p| step_of(p).is_none() && !(ids::MAT_BASE..ids::ARP_ON).contains(&p))
        {
            if widget(p) == Kind::Knob || widget(p) == Kind::Toggle {
                assert!(
                    short(p).chars().count() <= 9,
                    "{}: {}",
                    SPECS[p].id,
                    short(p)
                );
            }
        }
        assert_eq!(short(ids::MASTER_VOL), "Master");
        assert_eq!(short(ids::DLY_PP), "Ping");
    }

    #[test]
    fn a_fader_press_maps_to_the_range() {
        let b = Rect::new(0.0, 100.0, 30.0, 80.0);
        assert_eq!(fader_norm(b, 0.0), 1.0);
        assert_eq!(fader_norm(b, 1000.0), 0.0);
        assert!((fader_norm(b, 140.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn navigation_clamps_and_remembers() {
        let mut ui = Ui::default();
        ui.apply(Nav::Part(1));
        ui.apply(Nav::Tab(9));
        ui.apply(Nav::Fx(99));
        ui.apply(Nav::Page(5));
        ui.apply(Nav::Menu);
        assert_eq!(
            (ui.part, ui.tab, ui.fx, ui.page, ui.menu),
            (1, 3, 7, 1, true)
        );
        ui.apply(Nav::Menu);
        assert!(!ui.menu);
    }

    #[test]
    fn the_input_select_steps_through_a_b_and_both() {
        assert_eq!(route_name(1.0, 0.0), "A");
        assert_eq!(route_name(0.0, 1.0), "B");
        assert_eq!(route_name(1.0, 1.0), "AB");
        assert_eq!(route_name(0.5, 0.2), "R");
        assert_eq!(route_step(1.0, 0.0, 1), (0.0, 1.0));
        assert_eq!(route_step(0.0, 1.0, 1), (1.0, 1.0));
        assert_eq!(route_step(1.0, 1.0, 1), (1.0, 0.0));
        assert_eq!(route_step(1.0, 0.0, -1), (1.0, 1.0));
        // A mix of its own goes to the first (up) or last (down) route.
        assert_eq!(route_step(0.5, 0.2, 1), (1.0, 0.0));
        assert_eq!(route_step(0.5, 0.2, -1), (1.0, 1.0));
    }

    #[test]
    fn part_sync_mirrors_the_controls_the_parts_share() {
        use ids::{filt, osc, part};
        assert_eq!(mirror(osc_param(0, osc::VOL)), Some(osc_param(2, osc::VOL)));
        assert_eq!(
            mirror(osc_param(3, osc::DETUNE)),
            Some(osc_param(1, osc::DETUNE))
        );
        assert_eq!(
            mirror(part_param(0, part::ATT)),
            Some(part_param(1, part::ATT))
        );
        assert_eq!(
            mirror(part_param(1, part::REL)),
            Some(part_param(0, part::REL))
        );
        // A part's level, pan and switch are its own.
        assert_eq!(mirror(part_param(0, part::LEVEL)), None);
        assert_eq!(mirror(part_param(0, part::ON)), None);
        assert_eq!(
            mirror(filt_param(0, filt::CUTOFF)),
            Some(filt_param(1, filt::CUTOFF))
        );
        assert_eq!(mirror(filt_param(0, filt::IN_A)), None);
        assert_eq!(mirror(filt_param(1, filt::VOL)), None);
        assert_eq!(mirror(ids::MASTER_VOL), None);
        assert_eq!(mirror(ids::MAT_BASE), None);
    }

    #[test]
    fn the_keyboard_is_five_octaves_of_distinct_notes_with_the_black_keys_on_top() {
        let hits = layout(&Ui::default());
        let keys: Vec<_> = hits.iter().filter(|h| key(h)).collect();
        assert_eq!(keys.len(), 35 + 25);
        let notes: std::collections::BTreeSet<u8> = keys
            .iter()
            .map(|h| match h.kind {
                Kind::Key(n) => n,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(notes.len(), 60);
        assert_eq!(*notes.iter().next().unwrap(), KEY_FIRST);
        assert_eq!(*notes.iter().last().unwrap(), KEY_FIRST + 59);
        // The last hit at a point on a black key is the black key.
        for h in keys.iter().filter(|h| h.r.w < 9.0) {
            let top = hits
                .iter()
                .rev()
                .find(|o| o.r.contains(h.r.cx(), h.r.y + 2.0))
                .unwrap();
            assert_eq!(top.kind, h.kind);
        }
    }

    #[test]
    fn every_screen_paints_and_reports_the_regions_it_lays_out() {
        let mut c = Canvas::new(W, H, 1.0).unwrap();
        for ui in screens() {
            assert_eq!(paint(&mut c, &Defaults, &ui, Some(0), Some(1)), layout(&ui));
        }
        let open = Ui {
            popup: Some(mat_param(8, ids::mat::DST)),
            ..Ui::default()
        };
        assert_eq!(paint(&mut c, &Defaults, &open, None, None), layout(&open));
        let menu = Ui {
            menu: true,
            keys: 1 << 60 | 1 << 64,
            ..Ui::default()
        };
        assert_eq!(paint(&mut c, &Defaults, &menu, None, None), layout(&menu));
    }

    #[test]
    fn the_filter_control_is_its_own_filter() {
        for part in 0..2 {
            let ui = Ui {
                part,
                ..Ui::default()
            };
            let hits = layout(&ui);
            let count = |p: usize| hits.iter().filter(|h| h.param == p && h.is_param()).count();
            for p in [ids::GF_CUTOFF, ids::GF_RES, ids::GF_KEY] {
                assert_eq!(count(p), 1, "{}", ids::SPECS[p].id);
            }
            for f in [ids::filt::CUTOFF, ids::filt::RES] {
                assert_eq!(count(filt_param(part, f)), 1, "filter {part} field {f}");
            }
        }
    }
}
