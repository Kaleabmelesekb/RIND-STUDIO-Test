// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rind Studio's picture and its hit regions, in logical pixels: one screen, the whole synth in view. The oscillators
//! of the chosen part sit left and right of its amplitude envelope; below them the two filters, a display for the
//! effects, arpeggiator and last-touched value, and the mixer; below that the two modulation envelopes, the two LFOs
//! and two free modulation panels (the 16 matrix slots, a drop-down list for every source and target); a strip of
//! performance controls closes the screen. The look is its own: graphite panels, ember accents, a teal glass display.
//!
//! Everything is driven by the parameter table ([`crate::ids::SPECS`]); a page is a list of parameter indices.

use os_ui::{Canvas, Color, Font, Rect};

use crate::ids::{self, Fmt, Kind as Range, SPECS};

/// The editor's size at scale 1: the proportions of a 908 x 591 design.
pub const W: u32 = 800;
pub const H: u32 = 521;

/// Drag length in px for the whole range of a knob.
pub const KNOB_PX: f32 = 150.0;

/// A design pixel (of 908 x 591) in logical pixels.
const K: f32 = 800.0 / 908.0;

fn s(v: f32) -> f32 {
    v * K
}

/// A rect given in design pixels.
fn rr(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(s(x), s(y), s(w), s(h))
}

const BG: Color = Color::rgb(0x0b0e17);
const RAIL: Color = Color::rgb(0x11151f);
const PANEL: Color = Color::rgb(0x171c2a);
const HEAD_BG: Color = Color::rgb(0x212840);
const EDGE: Color = Color::rgb(0x313a57);
const HILITE: Color = Color::rgb(0x46517a);
const SLOT: Color = Color::rgb(0x080a11);
const KNOB_RIM: Color = Color::rgb(0x7782a8);
const KNOB_BODY: Color = Color::rgb(0x2a3150);
const CAP: Color = Color::rgb(0xdfe6ff);
const EMBER: Color = Color::rgb(0x35d6b8);
const EMBER_DIM: Color = Color::rgb(0x1c7f6d);
const GOLD: Color = Color::rgb(0xffc86b);
const MUTED: Color = Color::rgb(0x8e98b8);
const TICK: Color = Color::rgb(0x5a6488);
const LCD_BG: Color = Color::rgb(0x0a1428);
const LCD_FIELD: Color = Color::rgb(0x14264a);
const LCD_EDGE: Color = Color::rgb(0x3a5fa8);
const LCD_TEXT: Color = Color::rgb(0x9fd4ff);
const LCD_DIM: Color = Color::rgb(0x5c86c0);
const DARK_TEXT: Color = Color::rgb(0x06241e);
const SHADE: Color = Color::rgb(0x000000).with_alpha(0xaa);
const SHADOW: Color = Color::rgb(0x000000).with_alpha(0x70);
const SHEEN: Color = Color::rgb(0xffffff).with_alpha(0x1c);
const VEIL: Color = Color::rgb(0x0b0e17).with_alpha(0xb8);

const LABEL: Font = Font::new(7).bold();
const VALUE: Font = Font::new(8).bold();
const TITLE: Font = Font::new(8).bold();

/// What a tap on a navigation control changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    /// The part whose oscillators and envelope the screen shows.
    Part(usize),
    /// The display's tab.
    Tab(usize),
    /// The effect (or the arpeggiator) the display shows.
    Fx(usize),
    /// The arpeggiator's steps 1-8 or 9-16.
    Page(usize),
}

/// What the screen is showing, and the drop-down list open, if any (the parameter it picks).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ui {
    pub part: usize,
    pub tab: usize,
    pub fx: usize,
    pub page: usize,
    pub popup: Option<usize>,
}

impl Ui {
    pub fn apply(&mut self, nav: Nav) {
        match nav {
            Nav::Part(i) => self.part = i.min(1),
            Nav::Tab(i) => self.tab = i.min(DISPLAY_TABS.len() - 1),
            Nav::Fx(i) => self.fx = i.min(FX_NAMES.len() - 1),
            Nav::Page(i) => self.page = i.min(1),
        }
    }
}

const DISPLAY_TABS: [&str; 2] = ["FX", "LFO"];
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
    /// A whole-number box: a tap on its left half steps down, on its right half up.
    Stepper,
    /// A vertical fader: a press sets, a drag moves.
    Fader,
    /// A number cell: a vertical drag changes it.
    Num,
    /// Part, tab, effect or page buttons.
    Nav(Nav),
    /// Name number `i` in the open drop-down list.
    Item(usize),
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
    /// True for a control of a parameter (not chrome or a list).
    pub fn is_param(&self) -> bool {
        !matches!(self.kind, Kind::Nav(_) | Kind::Item(_) | Kind::Dismiss)
    }
}

/// The values the editor shows.
pub trait Values {
    /// Normalised.
    fn norm(&self, p: usize) -> f32;
    fn plain(&self, p: usize) -> f32;
    fn text(&self, p: usize) -> String;
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

/// True for a selector that opens a list (four names or more); a shorter one steps on a tap.
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
/// Share of a stepper's width at each end that steps (the middle drags).
pub const STEP_END: f32 = 0.26;

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
    Select,
    Led,
    Check,
    Stepper,
    Fader,
    Num,
    /// A navigation button with its caption.
    Button(&'static str),
}

struct Item {
    param: usize,
    kind: Kind,
    r: Rect,
    look: Look,
    /// The caption: above a select, switch or stepper, below a knob; empty for none.
    label: &'static str,
    /// Drawn selected (a navigation button).
    on: bool,
}

/// Text that is only drawn.
struct Text {
    x: f32,
    y: f32,
    s: &'static str,
    font: Font,
    color: Color,
    centered: bool,
}

#[derive(Default)]
struct Page {
    panels: Vec<(String, Rect)>,
    items: Vec<Item>,
    texts: Vec<Text>,
    /// New texts are drawn in the display's colours.
    lcd: bool,
}

impl Page {
    fn frame(&mut self, title: impl Into<String>, x: f32, y: f32, w: f32, h: f32) {
        self.panels.push((title.into(), rr(x, y, w, h)));
    }

    fn text(&mut self, x: f32, y: f32, s: &'static str) {
        self.texts.push(Text {
            x: self::s(x),
            y: self::s(y),
            s,
            font: LABEL,
            color: if self.lcd { LCD_DIM } else { MUTED },
            centered: false,
        });
    }

    fn text_c(&mut self, cx: f32, y: f32, s: &'static str) {
        self.texts.push(Text {
            x: self::s(cx),
            y: self::s(y),
            s,
            font: LABEL,
            color: if self.lcd { LCD_DIM } else { MUTED },
            centered: true,
        });
    }

    fn add(&mut self, param: usize, kind: Kind, r: Rect, look: Look, label: &'static str) {
        self.items.push(Item {
            param,
            kind,
            r,
            look,
            label,
            on: false,
        });
    }

    /// A knob centred at (`cx`, `cy`) in design pixels with a hit region `hw` design pixels either side and from
    /// `rad + 4` above to `below` under the centre.
    fn knob(&mut self, p: usize, cx: f32, cy: f32, rad: f32, hw: f32, label: &'static str) {
        let below = if label.is_empty() {
            rad + 4.0
        } else {
            rad + 15.0
        };
        let r = rr(cx - hw, cy - rad - 4.0, 2.0 * hw, rad + 4.0 + below);
        self.add(p, Kind::Knob, r, Look::Knob(rad), label);
    }

    /// A selector, a switch or a stepper by its parameter, in a rect of design pixels.
    fn ctl(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32) {
        let r = rr(x, y, w, h);
        match widget(p) {
            Kind::Cycle => self.add(p, Kind::Cycle, r, Look::Select, ""),
            Kind::Toggle => self.add(p, Kind::Toggle, r, Look::Led, ""),
            _ => self.add(p, Kind::Knob, r, Look::Knob(7.0), ""),
        }
    }

    fn stepper(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32) {
        self.add(p, Kind::Stepper, rr(x, y, w, h), Look::Stepper, "");
    }

    /// A parameter in a cell of design pixels (a knob with its label under it; a selector, switch or stepper with its
    /// label above).
    fn cell(&mut self, p: usize, x: f32, y: f32, w: f32, h: f32) {
        self.cell_as(p, short(p), x, y, w, h);
    }

    /// [`Page::cell`] with its own caption.
    fn cell_as(&mut self, p: usize, label: &'static str, x: f32, y: f32, w: f32, h: f32) {
        let cx = x + w / 2.0;
        match widget(p) {
            Kind::Knob if is_stepper(p) => {
                self.add(p, Kind::Stepper, rr(x, y, w, h), Look::Stepper, label);
            }
            Kind::Knob => self.knob(p, cx, y + 15.0, 11.0, w / 2.0, label),
            Kind::Cycle => self.add(p, Kind::Cycle, rr(x, y, w, h), Look::Select, label),
            _ => self.add(p, Kind::Toggle, rr(x, y, w, h), Look::Led, label),
        }
    }

    /// A target row of a matrix slot: its amount as a small knob and its target as a drop-down.
    fn row(&mut self, slot: usize, x: f32, y: f32, w: f32) {
        self.knob(
            mat_param(slot, ids::mat::AMT),
            x + 15.0,
            y + 9.5,
            7.0,
            10.0,
            "",
        );
        self.ctl(mat_param(slot, ids::mat::DST), x + 30.0, y, w - 36.0, 19.0);
    }

    fn nav(&mut self, nav: Nav, label: &'static str, on: bool, x: f32, y: f32, w: f32, h: f32) {
        self.items.push(Item {
            param: 0,
            kind: Kind::Nav(nav),
            r: rr(x, y, w, h),
            look: Look::Button(label),
            label: "",
            on,
        });
    }
}

/// True for a parameter drawn as a stepper (a small whole number: octave, note, voices, polyphony, bend, steps).
fn is_stepper(p: usize) -> bool {
    use ids::osc;
    let osc_at = |f: usize| (0..4).any(|n| p == ids::OSC_BASE + n * ids::OSC_STRIDE + f);
    p == ids::POLY
        || p == ids::BEND
        || p == ids::ARP_OCT
        || p == ids::ARP_STEPS
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

/// One oscillator panel at (`x`, `y`), 342 x 102 design pixels.
fn osc_panel(g: &mut Page, n: usize, x: f32, y: f32) {
    use ids::osc;
    g.frame(OSC_TITLES[n], x, y, 342.0, 102.0);
    g.text(x + 12.0, y + 21.0, "PITCH");
    g.text(x + 12.0, y + 38.0, "OCTAVE");
    g.stepper(osc_param(n, osc::OCT), x + 52.0, y + 33.0, 36.0, 18.0);
    g.text(x + 12.0, y + 59.0, "NOTE");
    g.stepper(osc_param(n, osc::SEMI), x + 52.0, y + 54.0, 36.0, 18.0);
    g.text(x + 12.0, y + 82.0, "FINE");
    g.knob(osc_param(n, osc::FINE), x + 66.0, y + 85.0, 7.0, 11.0, "");
    let knobs = [
        (osc::VOL, 118.0, "VOLUME"),
        (osc::PHASE, 166.0, "PHASE"),
        (osc::DETUNE, 214.0, "DETUNE"),
        (osc::STEREO, 262.0, "STEREO"),
        (osc::PAN, 310.0, "PAN"),
    ];
    for (f, dx, label) in knobs {
        g.knob(osc_param(n, f), x + dx, y + 43.0, 13.0, 24.0, label);
    }
    g.text(x + 86.0, y + 88.0, "INV");
    g.ctl(osc_param(n, osc::INV), x + 102.0, y + 80.0, 22.0, 16.0);
    g.ctl(osc_param(n, osc::WAVE), x + 130.0, y + 80.0, 60.0, 16.0);
    g.text(x + 196.0, y + 88.0, "VOICES");
    g.stepper(osc_param(n, osc::VOICES), x + 230.0, y + 80.0, 40.0, 16.0);
    g.text(x + 278.0, y + 88.0, "RAND");
    g.knob(osc_param(n, osc::RAND), x + 322.0, y + 88.0, 7.0, 11.0, "");
}

/// The filter panel of filter `n` at (`x`, `y`): `big` (the taller one) draws larger knobs.
fn filter_panel(g: &mut Page, n: usize, x: f32, y: f32, h: f32, big: bool) {
    use ids::filt;
    g.frame(format!("FILTER {}", n + 1), x, y, 218.0, h);
    g.ctl(filt_param(n, filt::ON), x + 178.0, y + 1.0, 36.0, 14.0);
    g.ctl(filt_param(n, filt::TYPE), x + 8.0, y + 20.0, 56.0, 17.0);
    g.ctl(filt_param(n, filt::SLOPE), x + 68.0, y + 20.0, 44.0, 17.0);
    g.text(x + 118.0, y + 25.0, "IN A");
    g.knob(
        filt_param(n, filt::IN_A),
        x + 146.0,
        y + 28.0,
        7.0,
        11.0,
        "",
    );
    g.text(x + 162.0, y + 25.0, "IN B");
    g.knob(
        filt_param(n, filt::IN_B),
        x + 190.0,
        y + 28.0,
        7.0,
        11.0,
        "",
    );
    let (cy, rad) = if big {
        (y + 74.0, 14.0)
    } else {
        (y + 58.0, 10.0)
    };
    let knobs = [
        (filt::CUTOFF, 30.0, "CUTOFF"),
        (filt::RES, 82.0, "RESONANCE"),
        (filt::DRIVE, 134.0, "DRIVE"),
        (filt::KEY, 186.0, "KEY TRACK"),
    ];
    for (f, dx, label) in knobs {
        g.knob(filt_param(n, f), x + dx, cy, rad, 25.0, label);
    }
}

fn mod_env_panel(g: &mut Page, n: usize, x: f32, y: f32, w: f32) {
    g.frame(format!("MOD ENV {}", n + 1), x, y, w, 156.0);
    for (i, letter) in ["A", "D", "S", "R"].into_iter().enumerate() {
        let cx = x + 27.0 + 27.0 * i as f32;
        g.text_c(cx, y + 22.0, letter);
        g.add(
            env_param(n, i),
            Kind::Fader,
            rr(cx - 10.0, y + 34.0, 20.0, 62.0),
            Look::Fader,
            "",
        );
    }
    g.row(n * 2, x, y + 104.0, w);
    g.row(n * 2 + 1, x, y + 130.0, w);
}

fn lfo_panel(g: &mut Page, n: usize, x: f32, y: f32, w: f32) {
    use ids::lfo;
    g.frame(format!("LFO {}", n + 1), x, y, w, 156.0);
    g.ctl(lfo_param(n, lfo::WAVE), x + 8.0, y + 20.0, 58.0, 17.0);
    g.text(x + 78.0, y + 27.0, "SYNC");
    g.ctl(lfo_param(n, lfo::SYNC), x + 102.0, y + 20.0, 28.0, 17.0);
    let knobs = [
        (lfo::RATE, 24.0, "RATE"),
        (lfo::GAIN, 69.0, "GAIN"),
        (lfo::OFFSET, 114.0, "OFFSET"),
    ];
    for (f, dx, label) in knobs {
        g.knob(lfo_param(n, f), x + dx, y + 60.0, 11.0, 21.0, label);
    }
    g.row(4 + n * 2, x, y + 104.0, w);
    g.row(5 + n * 2, x, y + 130.0, w);
}

fn misc_panel(g: &mut Page, n: usize, x: f32, y: f32, w: f32) {
    g.frame(format!("MISC {}", n + 1), x, y, w, 156.0);
    for group in 0..2 {
        let first = 8 + n * 4 + group * 2;
        let top = y + 20.0 + group as f32 * 66.0;
        g.text(x + 6.0, top + 6.0, "SRC");
        g.ctl(
            mat_param(first, ids::mat::SRC),
            x + 30.0,
            top,
            w - 36.0,
            17.0,
        );
        g.row(first, x, top + 22.0, w);
        g.row(first + 1, x, top + 44.0, w);
    }
}

/// The fixed part of the screen.
fn front(g: &mut Page, ui: &Ui) {
    // The top bar.
    g.text(62.0, 9.0, "POLYPHONY");
    g.stepper(ids::POLY, 124.0, 3.0, 44.0, 21.0);
    g.text(214.0, 9.0, "VOICE MODE");
    g.ctl(ids::MONO, 270.0, 3.0, 60.0, 21.0);
    g.text(352.0, 9.0, "PART SELECT");
    g.nav(Nav::Part(0), "PART A", ui.part == 0, 412.0, 3.0, 52.0, 21.0);
    g.nav(Nav::Part(1), "PART B", ui.part == 1, 468.0, 3.0, 52.0, 21.0);
    g.text(540.0, 9.0, "PART ON");
    g.ctl(part_param(ui.part, ids::part::ON), 584.0, 3.0, 34.0, 21.0);

    // The oscillators of the part and its amplitude envelope.
    let (a, b) = (ui.part * 2, ui.part * 2 + 1);
    osc_panel(g, a, 30.0, 30.0);
    osc_panel(g, b, 537.0, 30.0);
    g.frame(
        format!("AMP ENV {}", ["A", "B"][ui.part]),
        376.0,
        30.0,
        158.0,
        102.0,
    );
    for (i, letter) in ["A", "D", "S", "R"].into_iter().enumerate() {
        let cx = 413.0 + 26.5 * i as f32;
        g.text_c(cx, 50.0, letter);
        g.add(
            part_param(ui.part, ids::part::ATT + i),
            Kind::Fader,
            rr(cx - 10.0, 60.0, 20.0, 64.0),
            Look::Fader,
            "",
        );
    }

    filter_panel(g, 0, 30.0, 135.0, 117.0, true);
    filter_panel(g, 1, 30.0, 258.0, 87.0, false);

    // The mixer.
    g.frame("MIXER", 658.0, 135.0, 222.0, 127.0);
    let faders = [
        (part_param(0, ids::part::LEVEL), 686.0, "MIX A"),
        (part_param(1, ids::part::LEVEL), 722.0, "MIX B"),
        (ids::MASTER_VOL, 766.0, "MAIN VOL"),
    ];
    for (p, cx, label) in faders {
        g.text_c(cx, 154.0, label);
        g.add(
            p,
            Kind::Fader,
            rr(cx - 10.0, 166.0, 20.0, 88.0),
            Look::Fader,
            "",
        );
    }
    let mini = [
        (filt_param(0, ids::filt::VOL), "F1 VOL", 806.0, 160.0),
        (filt_param(0, ids::filt::PAN), "F1 PAN", 842.0, 160.0),
        (filt_param(1, ids::filt::VOL), "F2 VOL", 806.0, 206.0),
        (filt_param(1, ids::filt::PAN), "F2 PAN", 842.0, 206.0),
    ];
    for (p, label, x, y) in mini {
        g.cell_as(p, label, x, y, 34.0, 44.0);
    }

    // The bottom panels.
    mod_env_panel(g, 0, 30.0, 352.0, 138.0);
    mod_env_panel(g, 1, 173.0, 352.0, 139.0);
    lfo_panel(g, 0, 316.0, 352.0, 139.0);
    lfo_panel(g, 1, 460.0, 352.0, 137.0);
    misc_panel(g, 0, 600.0, 352.0, 137.0);
    misc_panel(g, 1, 741.0, 352.0, 139.0);

    // The performance strip.
    g.frame("PERFORMANCE", 30.0, 512.0, 850.0, 72.0);
    let strip = [
        ids::BEND,
        ids::MASTER_TUNE,
        ids::VEL_AMP,
        ids::PORTA,
        ids::PORTA_MODE,
        part_param(0, ids::part::PAN),
        part_param(1, ids::part::PAN),
    ];
    for (i, p) in strip.into_iter().enumerate() {
        g.cell(p, 70.0 + 112.0 * i as f32, 528.0, 96.0, 52.0);
    }
}

/// The display's contents.
fn display(g: &mut Page, ui: &Ui) {
    g.lcd = true;
    // The bezel and the tabs; the screen is drawn in `paint_display`.
    for (i, name) in DISPLAY_TABS.iter().enumerate() {
        g.nav(
            Nav::Tab(i),
            name,
            ui.tab == i,
            272.0 + 40.0 * i as f32,
            148.0,
            38.0,
            18.0,
        );
    }
    if ui.tab == 1 {
        g.text(284.0, 214.0, "LFO 1");
        g.text(284.0, 268.0, "LFO 2");
        for n in 0..2 {
            let y = if n == 0 { 202.0 } else { 256.0 };
            use ids::lfo;
            for (i, f) in [lfo::DIV, lfo::PHASE, lfo::RETRIG].into_iter().enumerate() {
                g.cell(lfo_param(n, f), 330.0 + 76.0 * i as f32, y, 64.0, 46.0);
            }
        }
        return;
    }
    // The effect list: a switch and a name to each.
    for (i, name) in FX_NAMES.iter().enumerate() {
        let y = 196.0 + 16.0 * i as f32;
        g.add(
            FX_ON[i],
            Kind::Toggle,
            rr(268.0, y, 20.0, 16.0),
            Look::Check,
            "",
        );
        g.nav(Nav::Fx(i), name, ui.fx == i, 290.0, y, 70.0, 16.0);
    }
    if ui.fx != 0 {
        for (i, &p) in FX_PARAMS[ui.fx].iter().enumerate() {
            let (col, row) = (i % 6, i / 6);
            g.cell(
                p,
                366.0 + 42.0 * col as f32,
                198.0 + 56.0 * row as f32,
                40.0,
                52.0,
            );
        }
        return;
    }
    // The arpeggiator.
    g.text(368.0, 202.0, "MODE");
    g.ctl(ids::ARP_MODE, 404.0, 196.0, 84.0, 16.0);
    g.text(368.0, 222.0, "VEL");
    g.ctl(ids::ARP_VELMODE, 404.0, 216.0, 84.0, 16.0);
    g.text(368.0, 242.0, "OCTAVE");
    g.stepper(ids::ARP_OCT, 404.0, 236.0, 52.0, 16.0);
    g.text(368.0, 262.0, "STEPS");
    g.stepper(ids::ARP_STEPS, 404.0, 256.0, 52.0, 16.0);
    g.text(498.0, 202.0, "RATE");
    g.ctl(ids::ARP_DIV, 524.0, 196.0, 66.0, 16.0);
    g.text(498.0, 222.0, "HOLD");
    g.ctl(ids::ARP_HOLD, 524.0, 216.0, 44.0, 16.0);
    g.knob(ids::ARP_GATE, 596.0, 228.0, 11.0, 24.0, "GATE");
    g.text(500.0, 262.0, "PAGE");
    g.nav(Nav::Page(0), "1-8", ui.page == 0, 524.0, 256.0, 40.0, 16.0);
    g.nav(Nav::Page(1), "9-16", ui.page == 1, 568.0, 256.0, 44.0, 16.0);
    g.text(366.0, 282.0, "HLD");
    g.text(366.0, 301.0, "TRN");
    g.text(366.0, 321.0, "VEL");
    for i in 0..8 {
        let step = ui.page * 8 + i;
        let x = 392.0 + 28.5 * i as f32;
        g.add(
            ids::ARP_HOLD1 + step,
            Kind::Toggle,
            rr(x, 276.0, 27.0, 18.0),
            Look::Led,
            "",
        );
        g.add(
            ids::ARP_TR1 + step,
            Kind::Num,
            rr(x, 296.0, 27.0, 18.0),
            Look::Num,
            "",
        );
        g.add(
            ids::ARP_STEP1 + step,
            Kind::Num,
            rr(x, 316.0, 27.0, 18.0),
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
    rr(257.0, 135.0, 373.0, 207.0)
}

fn screen_rect() -> Rect {
    rr(264.0, 142.0, 359.0, 193.0)
}

// ---- the drop-down list ----------------------------------------------------------------------------------------

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

/// Every hit region of `ui`'s screen, the last on top (no painting). With a list open, only the list and the rest of
/// the window that closes it.
pub fn layout(ui: &Ui) -> Vec<Hit> {
    let pg = page(ui);
    if let Some(p) = ui.popup {
        let anchor = pg.items.iter().find(|i| i.param == p).map(|i| i.r);
        let mut hits = vec![Hit {
            param: 0,
            kind: Kind::Dismiss,
            r: Rect::new(0.0, 0.0, W as f32, H as f32),
        }];
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
    for (title, r) in &pg.panels {
        paint_panel(c, title, *r);
    }
    paint_plate(c);
    paint_display(c, v, last);
    for t in &pg.texts {
        let w = if t.centered {
            c.text_width(t.font, t.s)
        } else {
            0.0
        };
        c.text(t.font, (t.x - w / 2.0).round(), t.y.round(), t.color, t.s);
    }
    for it in &pg.items {
        paint_item(c, v, it, pressed == Some(it.param));
    }
    if let Some(p) = ui.popup
        && let Some(anchor) = pg.items.iter().find(|i| i.param == p).map(|i| i.r)
    {
        paint_popup(c, v, p, anchor);
    }
    layout(ui)
}

/// The window's frame: the side rails and the top bar.
fn paint_frame(c: &mut Canvas) {
    let (w, h) = (W as f32, H as f32);
    c.fill_rect(Rect::new(0.0, 0.0, s(27.0), h), RAIL);
    c.fill_rect(Rect::new(w - s(27.0), 0.0, s(27.0), h), RAIL);
    c.fill_rect(Rect::new(s(26.0), 0.0, 1.5, h), EMBER_DIM);
    c.fill_rect(Rect::new(w - s(27.0) - 0.5, 0.0, 1.5, h), EMBER_DIM);
    c.fill_rect(Rect::new(0.0, 0.0, w, s(27.0)), RAIL);
    c.fill_rect(Rect::new(0.0, s(27.0) - 1.0, w, 1.0), EDGE);
    let brand = Rect::new(w - s(190.0), 0.0, s(160.0), s(27.0));
    c.text_center(Font::new(11).bold(), brand, EMBER, "RIND STUDIO");
}

fn paint_panel(c: &mut Canvas, title: &str, r: Rect) {
    c.fill_rrect(r.inset(-1.0), 6.0, SLOT);
    c.fill_rrect(r, 5.0, PANEL);
    c.stroke_rrect(r, 5.0, 1.0, EDGE);
    let head = Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, s(15.0));
    c.fill_rrect(head, 4.0, HEAD_BG);
    c.fill_rect(Rect::new(r.x + 3.0, r.y + 1.0, r.w - 6.0, 1.0), HILITE);
    c.text(TITLE, r.x + 8.0, r.y + 3.0, GOLD, title);
}

/// The name plate under the mixer.
fn paint_plate(c: &mut Canvas) {
    let r = rr(658.0, 268.0, 222.0, 77.0);
    c.fill_rrect(r, 6.0, SLOT);
    c.stroke_rrect(r, 6.0, 1.0, EDGE);
    let word = Rect::new(r.x, r.y + s(8.0), r.w, s(40.0));
    c.text_center(Font::new(26).bold(), word, EMBER, "Rind Studio");
    let sub = Rect::new(r.x, r.y + s(52.0), r.w, s(14.0));
    c.text_center(Font::new(9), sub, MUTED, "MATRIX SYNTHESIZER");
}

fn paint_display(c: &mut Canvas, v: &dyn Values, last: Option<usize>) {
    let d = display_rect();
    c.fill_rrect(d.inset(-1.0), 8.0, SLOT);
    c.fill_rrect(d, 7.0, HEAD_BG);
    c.stroke_rrect(d, 7.0, 1.0, HILITE);
    let sc = screen_rect();
    c.fill_rrect(sc, 4.0, LCD_BG);
    c.stroke_rrect(sc, 4.0, 1.5, LCD_EDGE);
    // The title, and the value last touched.
    let title = Rect::new(sc.x + s(96.0), s(148.0), sc.w - s(110.0), s(18.0));
    c.text_center(VALUE, title, LCD_TEXT, "RIND STUDIO");
    let line = Rect::new(sc.x + s(6.0), s(170.0), sc.w - s(12.0), s(20.0));
    c.fill_rrect(line, 3.0, LCD_FIELD);
    c.stroke_rrect(line, 3.0, 1.0, LCD_EDGE);
    match last {
        Some(p) => {
            c.text(VALUE, line.x + 6.0, line.y + s(6.0), LCD_DIM, SPECS[p].name);
            c.text_right(
                VALUE,
                line.right() - 6.0,
                line.y + s(6.0),
                LCD_TEXT,
                &v.text(p),
            );
        }
        None => {
            c.text(
                VALUE,
                line.x + 6.0,
                line.y + s(6.0),
                LCD_DIM,
                "Touch a control to read it here",
            );
        }
    }
}

/// The matrix slot of a target or amount parameter.
fn slot_of(p: usize) -> Option<usize> {
    let end = ids::MAT_BASE + 16 * ids::MAT_STRIDE;
    (p >= ids::MAT_BASE && p < end).then(|| (p - ids::MAT_BASE) / ids::MAT_STRIDE)
}

fn paint_item(c: &mut Canvas, v: &dyn Values, it: &Item, held: bool) {
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
        Look::Select => paint_select(c, v, it.param, ctrl, held),
        Look::Led => paint_led(c, v, it.param, ctrl),
        Look::Check => paint_check(c, v, it.param, ctrl),
        Look::Stepper => paint_stepper(c, v, it.param, ctrl, held),
        Look::Fader => paint_fader(c, v, it, held),
        Look::Num => paint_num(c, v, it.param, ctrl, held),
        Look::Button(name) => paint_button(c, name, it.r, it.on),
    }
    if !it.label.is_empty() && !matches!(it.look, Look::Knob(_)) {
        let text = it.label.to_uppercase();
        let f = c.fit_font(LABEL, &text, it.r.w);
        let w = c.text_width(f, &text);
        c.text(f, (it.r.cx() - w / 2.0).round(), it.r.y, MUTED, &text);
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
    let top = s(11.0);
    let h = (r.h - top).min(s(18.0));
    let w = match it.look {
        Look::Stepper => (r.w - 4.0).min(s(58.0)),
        Look::Led => (r.w - 4.0).min(s(36.0)),
        _ => r.w - 4.0,
    };
    Rect::new(r.cx() - w / 2.0, r.y + top, w, h)
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
    for i in 0..=10 {
        let a = start + sweep * i as f32 / 10.0;
        let (sa, ca) = a.sin_cos();
        c.line(
            cx + (rad + 2.5) * ca,
            cy + (rad + 2.5) * sa,
            cx + (rad + 4.5) * ca,
            cy + (rad + 4.5) * sa,
            0.8,
            TICK,
        );
    }
    if bipolar(it.param) {
        let (mid, a) = (start + sweep * 0.5, start + sweep * n);
        if (a - mid).abs() > 0.01 {
            c.arc(cx, cy, rad + 2.0, mid.min(a), (a - mid).abs(), 1.8, EMBER);
        }
    } else if n > 0.002 {
        c.arc(cx, cy, rad + 2.0, start, sweep * n, 1.8, EMBER);
    }
    c.fill_circle(cx + 0.8, cy + 1.4, rad, SHADOW);
    c.fill_circle(cx, cy, rad, if held { EMBER_DIM } else { KNOB_RIM });
    c.fill_circle(cx, cy, rad - 1.3, KNOB_BODY);
    c.fill_circle(cx - rad * 0.22, cy - rad * 0.3, rad * 0.5, SHEEN);
    let a = start + sweep * n;
    let (sa, ca) = a.sin_cos();
    c.line(
        cx + rad * 0.3 * ca,
        cy + rad * 0.3 * sa,
        cx + rad * 0.85 * ca,
        cy + rad * 0.85 * sa,
        1.6,
        CAP,
    );
    if !it.label.is_empty() {
        let text = it.label.to_uppercase();
        let f = c.fit_font(LABEL, &text, it.r.w - 1.0);
        let w = c.text_width(f, &text);
        c.text(
            f,
            (cx - w / 2.0).round(),
            (cy + rad + s(5.0)).round(),
            MUTED,
            &text,
        );
    }
}

fn chevron(c: &mut Canvas, x: f32, y: f32) {
    c.polyline(
        &[(x - 2.5, y - 1.2), (x, y + 1.6), (x + 2.5, y - 1.2)],
        1.2,
        LCD_DIM,
    );
}

fn paint_select(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, held: bool) {
    c.fill_rrect(r, 3.0, if held { LCD_EDGE } else { LCD_FIELD });
    c.stroke_rrect(r, 3.0, 1.0, LCD_EDGE);
    let text = v.text(p);
    let mut body = Rect::new(r.x + 2.0, r.y, r.w - 12.0, r.h);
    if is_wave(p) {
        wave_icon(
            c,
            v.plain(p).round() as i32,
            Rect::new(r.x + 4.0, r.y + 3.0, 14.0, r.h - 6.0),
        );
        body = Rect::new(r.x + 20.0, r.y, r.w - 30.0, r.h);
    }
    let f = c.fit_font(VALUE, &text, body.w);
    c.text_center(f, body, LCD_TEXT, &text);
    if dropdown(p) {
        chevron(c, r.right() - 6.0, r.cy());
    }
}

fn paint_led(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect) {
    let on = v.plain(p) > 0.5;
    if on {
        c.fill_rrect(r.inset(-1.5), 5.0, EMBER_DIM);
    }
    c.fill_rrect(r, 4.0, if on { EMBER } else { SLOT });
    c.stroke_rrect(r, 4.0, 1.0, if on { GOLD } else { EDGE });
    if on {
        c.text_center(LABEL, r, DARK_TEXT, "ON");
    } else {
        c.text_center(LABEL, r, TICK, "OFF");
    }
}

fn paint_check(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect) {
    let on = v.plain(p) > 0.5;
    let b = Rect::new(r.x + 3.0, r.cy() - 5.0, 10.0, 10.0);
    c.fill_rrect(b, 2.0, if on { LCD_TEXT } else { LCD_FIELD });
    c.stroke_rrect(b, 2.0, 1.0, LCD_EDGE);
    if on {
        c.polyline(
            &[
                (b.x + 2.0, b.cy()),
                (b.x + 4.2, b.bottom() - 2.5),
                (b.right() - 2.0, b.y + 2.5),
            ],
            1.5,
            LCD_BG,
        );
    }
}

fn paint_stepper(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, held: bool) {
    c.fill_rrect(r, 4.0, if held { LCD_EDGE } else { LCD_FIELD });
    c.stroke_rrect(r, 4.0, 1.0, LCD_EDGE);
    // The arrows are the ends of the box: a press on either third steps, the middle drags.
    let end = (r.w * 0.26).min(r.h);
    c.fill_rect(Rect::new(r.x + end, r.y + 2.0, 1.0, r.h - 4.0), LCD_EDGE);
    c.fill_rect(
        Rect::new(r.right() - end - 1.0, r.y + 2.0, 1.0, r.h - 4.0),
        LCD_EDGE,
    );
    let text = v.text(p);
    let body = Rect::new(r.x + end + 1.0, r.y, r.w - 2.0 * end - 2.0, r.h);
    let f = c.fit_font(VALUE, &text, body.w);
    c.text_center(f, body, LCD_TEXT, &text);
    let (y, a) = (r.cy(), (r.h * 0.26).clamp(3.0, 6.0));
    let lx = r.x + end / 2.0;
    let rx = r.right() - end / 2.0;
    c.polyline(
        &[
            (lx + a * 0.6, y - a),
            (lx - a * 0.6, y),
            (lx + a * 0.6, y + a),
        ],
        2.2,
        EMBER,
    );
    c.polyline(
        &[
            (rx - a * 0.6, y - a),
            (rx + a * 0.6, y),
            (rx - a * 0.6, y + a),
        ],
        2.2,
        EMBER,
    );
}

fn paint_fader(c: &mut Canvas, v: &dyn Values, it: &Item, held: bool) {
    let n = v.norm(it.param).clamp(0.0, 1.0);
    let r = it.r;
    let cx = r.cx();
    let (top, bot) = (r.y + FADER_PAD, r.bottom() - FADER_PAD);
    for i in 0..=10 {
        let y = top + (bot - top) * i as f32 / 10.0;
        c.line(cx - 9.0, y, cx - 6.0, y, 0.8, TICK);
        c.line(cx + 6.0, y, cx + 9.0, y, 0.8, TICK);
    }
    c.fill_rrect(Rect::new(cx - 2.0, top, 4.0, bot - top), 2.0, SLOT);
    let y = top + (1.0 - n) * (bot - top);
    c.fill_rrect(
        Rect::new(cx - 1.0, y, 2.0, (bot - y).max(0.5)),
        1.0,
        EMBER_DIM,
    );
    let cap = Rect::new(cx - 7.0, y - 5.0, 14.0, 10.0);
    c.fill_rrect(Rect::new(cx - 8.0, y - 4.0, 16.0, 12.0), 3.0, SHADOW);
    c.fill_rrect(cap, 3.0, if held { GOLD } else { CAP });
    c.line(cx - 5.0, y, cx + 5.0, y, 1.4, EMBER);
}

fn num_text(v: &dyn Values, p: usize) -> String {
    if is_int(p) {
        format!("{:+}", v.plain(p).round() as i32)
    } else {
        format!("{}", (v.plain(p) * 100.0).round() as i32)
    }
}

fn paint_num(c: &mut Canvas, v: &dyn Values, p: usize, r: Rect, held: bool) {
    let live = step_of(p).is_none_or(|st| (st as f32) < v.plain(ids::ARP_STEPS));
    c.fill_rrect(r, 3.0, if held { LCD_EDGE } else { LCD_FIELD });
    c.stroke_rrect(r, 3.0, 1.0, LCD_EDGE);
    c.text_center(
        VALUE,
        r,
        if live { LCD_TEXT } else { LCD_DIM },
        &num_text(v, p),
    );
}

fn paint_button(c: &mut Canvas, name: &str, r: Rect, on: bool) {
    c.fill_rrect(r, 4.0, if on { EMBER } else { LCD_FIELD });
    c.stroke_rrect(r, 4.0, 1.0, if on { GOLD } else { LCD_EDGE });
    let f = c.fit_font(VALUE, name, r.w - 4.0);
    c.text_center(f, r, if on { DARK_TEXT } else { LCD_TEXT }, name);
}

fn is_wave(p: usize) -> bool {
    let end = ids::OSC_BASE + 4 * ids::OSC_STRIDE;
    p >= ids::OSC_BASE && p < end && (p - ids::OSC_BASE) % ids::OSC_STRIDE == ids::osc::WAVE
}

/// The step an arpeggiator per-step parameter belongs to (0-based).
fn step_of(p: usize) -> Option<usize> {
    [ids::ARP_STEP1, ids::ARP_TR1, ids::ARP_HOLD1]
        .iter()
        .find(|&&b| p >= b && p < b + 16)
        .map(|&b| p - b)
}

fn paint_popup(c: &mut Canvas, v: &dyn Values, p: usize, anchor: Rect) {
    c.fill_rect(Rect::new(0.0, 0.0, W as f32, H as f32), SHADE);
    let (names, lo) = choices(p);
    let (all, rects) = popup_rects(anchor, names.len());
    c.fill_rrect(all.inset(-2.0), 7.0, EMBER);
    c.fill_rrect(all, 6.0, LCD_BG);
    let now = v.plain(p).round() as i32 - lo;
    for (i, (r, name)) in rects.iter().zip(names).enumerate() {
        let on = i as i32 == now;
        if on {
            c.fill_rrect(r.inset(1.5), 4.0, EMBER);
        }
        let f = c.fit_font(VALUE, name, r.w - 10.0);
        c.text_center(f, *r, if on { DARK_TEXT } else { LCD_TEXT }, name);
    }
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
const SHORT: [(&str, &str); 38] = [
    ("Master Volume", "Master"),
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
    c.polyline(&pts, 1.3, GOLD);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every screen: each part, with the display on each effect, each arpeggiator page and the LFO tab.
    fn screens() -> Vec<Ui> {
        let mut out = Vec::new();
        for part in 0..2 {
            for fx in 0..FX_NAMES.len() {
                for page in 0..if fx == 0 { 2 } else { 1 } {
                    out.push(Ui {
                        part,
                        tab: 0,
                        fx,
                        page,
                        popup: None,
                    });
                }
            }
            out.push(Ui {
                part,
                tab: 1,
                ..Ui::default()
            });
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

    #[test]
    fn every_screen_lays_out_inside_the_window_without_overlap() {
        for ui in screens() {
            let hits = layout(&ui);
            for (i, a) in hits.iter().enumerate() {
                assert!(inside(window(), a.r), "{ui:?}: {a:?} leaves the window");
                for b in &hits[i + 1..] {
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
                    _ => (14.0, 12.0),
                };
                assert!(h.r.w >= w && h.r.h >= hh, "{ui:?}: {h:?}");
            }
        }
    }

    #[test]
    fn every_parameter_has_a_control_on_some_screen() {
        let mut seen = vec![false; ids::COUNT];
        for ui in screens() {
            for h in layout(&ui) {
                if h.is_param() {
                    seen[h.param] = true;
                }
            }
        }
        // A source the screen sets for its rows has no control of its own.
        let missing: Vec<_> = (0..ids::COUNT)
            .filter(|&i| !seen[i] && !derived_source(i))
            .map(|i| SPECS[i].id)
            .collect();
        assert!(missing.is_empty(), "no control for {missing:?}");
    }

    #[test]
    fn no_parameter_has_two_controls_on_one_screen() {
        for ui in screens() {
            let mut seen = std::collections::HashSet::new();
            for h in layout(&ui).iter().filter(|h| h.is_param()) {
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
                .filter(|h| h.is_param() && dropdown(h.param))
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
    fn selectors_with_many_names_are_drop_downs_and_short_ones_step() {
        assert!(dropdown(mat_param(8, ids::mat::SRC)));
        assert!(dropdown(mat_param(0, ids::mat::DST)));
        assert!(dropdown(osc_param(0, ids::osc::WAVE)));
        assert!(dropdown(ids::ARP_MODE));
        assert!(!dropdown(ids::MONO));
        assert!(!dropdown(filt_param(0, ids::filt::SLOPE)));
        assert!(!dropdown(filt_param(0, ids::filt::TYPE)));
        assert!(!dropdown(ids::MASTER_VOL));
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
        assert_eq!((ui.part, ui.tab, ui.fx, ui.page), (1, 1, 7, 1));
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
    }
}
