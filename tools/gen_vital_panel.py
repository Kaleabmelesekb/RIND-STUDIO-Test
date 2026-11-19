#!/usr/bin/env python3
"""Generates `src/panels/inst/vital.rs`, the Vital card, from the host parameter table.

Every param of `src/params/vital_table.rs` (but Enable) gets exactly one control: the script fails when a name it
binds is missing from the table, or a table param is left without a control. Run from the repository root:

    python3 tools/gen_vital_panel.py
"""
import re
import sys

TABLE = open("src/params/vital_table.rs").read()
body = TABLE[TABLE.index("pub static PARAMS") :]
ROWS = re.findall(r'^\s*P::(\w+)\(\s*"([^"]+)"', body, re.M)
KIND = {n: k for k, n in ROWS}
NAMES = [n for _, n in ROWS]
bound = set()
used_ids = set()
next_id = [10]


def nid():
    next_id[0] += 1
    return next_id[0]


def esc(s):
    return s.replace('"', '\\"')


def bind(name):
    if name not in KIND:
        sys.exit(f"no param {name!r}")
    if name in bound:
        sys.exit(f"param {name!r} bound twice")
    bound.add(name)
    return f'ix("{esc(name)}")'


def knob(label, name):
    assert KIND[name] != "toggle", name
    return f'knob({nid()}, "{esc(label)}").p({bind(name)})'


class S(str):
    """A control expression that can take a flexible width."""

    def w_flex(self, f):
        return S(f"{self}.w(Size::Flex({f}))")


def spin(label, name, per=0):
    k = KIND[name]
    assert k == "menu", name
    cols = f".items_per_column({per})" if per else ""
    return S(f'spinner({nid()}, "{esc(label)}").p({bind(name)}){cols}')


def tog(label, name):
    assert KIND[name] == "toggle", name
    return f'toggle({nid()}, "{esc(label)}").p({bind(name)})'


def vrow(*items, h=None):
    """A row of controls, `h` U tall or flexible."""
    inner = ",\n".join(items)
    size = f".h(Size::U({h}))" if h else ""
    return f"hstack([\n{inner},\n]){size}"


def knobs(prefix, pairs, per=None):
    return vrow(*[knob(l, f"{prefix}{n}") for l, n in pairs])


CONSTS = []


def list_items(name, items):
    """A const for the items of a list, split over lines so the file stays formattable."""
    chunks = [" · ".join(items[i : i + 5]) for i in range(0, len(items), 5)]
    pieces = [c + (" · " if k + 1 < len(chunks) else "") for k, c in enumerate(chunks)]
    if len(pieces) == 1:
        CONSTS.append(f'const {name}: &str = "{pieces[0]}";\n')
        return name
    lines = ",\n".join(f'    "{c}"' for c in pieces)
    CONSTS.append(f"const {name}: &str = concat!(\n{lines}\n);\n")
    return name


def listed(items, pages, w=2.4):
    """A param-less list at the left selecting one of `pages` (one Part each)."""
    lid = nid()
    labels = list_items(f"LIST_{lid}", items)
    parts = [f'vstack([list({lid}, {labels}, Bar::Right)]).w(Size::U({w}))']
    for k, p in enumerate(pages):
        parts.append(f"{p}.show(Show::Setting({lid}, {k}))")
    # `pages` are vstack(...) without a show; the list's page choice is applied here.
    return "hstack([\n" + ",\n".join(parts) + ",\n])"


FUNCS = []


def func(name, expr):
    """Emits `expr` as a function of its own, so no expression nests deeply; returns the call."""
    FUNCS.append(f"fn {name}() -> Part {{\n{expr}\n}}\n")
    return f"{name}()"


def plain(*rows):
    return "vstack([\n" + ",\n".join(rows) + ",\n])"


# ---- OSC ----
def osc(o):
    n = f"Oscillator {o} "
    return func(f"osc_{o}", plain(
        vrow(
            spin("Wavetable", n + "Wavetable"),
            spin("Stack", n + "Stack Style"),
            tog("On", n + "Switch"),
            tog("Midi Track", n + "Midi Track"),
            tog("Smooth", n + "Smooth Interpolation"),
            tog("Spectral", n + "Spectral Unison"),
            h=1.25,
        ),
        knobs(n, [("Level", "Level"), ("Pan", "Pan"), ("Transpose", "Transpose"), ("Tune", "Tune"), ("Frame", "Wave Frame"), ("Blend", "Blend")]),
        knobs(n, [("Voices", "Unison Voices"), ("Detune", "Unison Detune"), ("Power", "Detune Power"), ("Range", "Detune Range"), ("Stereo", "Stereo Spread"), ("Spread", "Unison Frame Spread")]),
        knobs(n, [("Phase", "Phase"), ("Rand Phase", "Phase Randomization"), ("Dist", "Distortion Amount"), ("Dist Phase", "Distortion Phase"), ("Dist Spread", "Distortion Spread")]),
        vrow(
            spin("Distortion", n + "Distortion Type"),
            spin("Morph", n + "Frequency Morph Type"),
            spin("Output", n + "Destination", 15),
            h=1.25,
        ),
        knobs(n, [("Quantize", "Transpose Quantize"), ("Morph", "Frequency Morph Amount"), ("Morph Spread", "Frequency Morph Spread")]),
    ))


def sample():
    n = "Sample "
    return func(f"sample", plain(
        vrow(
            tog("On", n + "Switch"),
            tog("Key Track", n + "Keytrack"),
            tog("Loop", n + "Loop"),
            tog("Rand Phase", n + "Random Phase"),
            tog("Bounce", n + "Bounce"),
            spin("Output", n + "Destination", 15),
            h=1.25,
        ),
        knobs(n, [("Level", "Level"), ("Pan", "Pan"), ("Transpose", "Transpose"), ("Tune", "Tune"), ("Quantize", "Transpose Quantize")]),
    ))


def sub():
    n = "Sub "
    return func(f"sub", plain(
        vrow(tog("On", n + "Switch"), spin("Waveform", n + "Osc Waveform"), h=1.25),
        knobs(n, [("Level", "Level"), ("Pan", "Pan"), ("Transpose", "Transpose"), ("Tune", "Tune"), ("Quantize", "Transpose Quantize"), ("Direct", "Direct Out")]),
    ))


def osc_tab():
    return listed(
        ["OSC 1", "OSC 2", "OSC 3", "SAMPLE", "SUB"],
        [osc(1), osc(2), osc(3), sample(), sub()],
    )


# ---- FILTER ----
def filt(prefix):
    n = prefix + " "
    return func("filter_" + prefix.split(" ", 1)[1], plain(
        vrow(tog("On", n + "Switch"), spin("Model", n + "Model"), spin("Style", n + "Style"), h=1.25),
        vrow(
            tog("Osc 1", n + "OSC 1 Input"),
            tog("Osc 2", n + "OSC 2 Input"),
            tog("Osc 3", n + "OSC 3 Input"),
            tog("Sample", n + "SAMPLE Input"),
            tog("Filter", n + "FILTER Input"),
            h=1.25,
        ),
        knobs(n, [("Cutoff", "Cutoff"), ("Resonance", "Resonance"), ("Drive", "Drive"), ("Blend", "Blend"), ("Mix", "Mix"), ("Key Track", "Key Track")]),
        knobs(n, [("Form X", "Formant X"), ("Form Y", "Formant Y"), ("Form Res", "Formant Resonance"), ("Form Trans", "Formant Transpose"), ("Form Spread", "Formant Spread"), ("Comb", "Comb Blend Offset")]),
    ))


def filter_tab():
    return listed(["FILTER 1", "FILTER 2"], [filt("Filter 1"), filt("Filter 2")])


# ---- ENV ----
def env(e):
    n = f"Envelope {e} "
    return func(f"envelope_{e}", plain(
        knobs(n, [("Delay", "Delay"), ("Attack", "Attack"), ("Hold", "Hold"), ("Decay", "Decay"), ("Sustain", "Sustain"), ("Release", "Release")]),
        knobs(n, [("Attack Curve", "Attack Power"), ("Decay Curve", "Decay Power"), ("Release Curve", "Release Power")]),
    ))


def env_tab():
    return listed([f"ENV {e}" for e in range(1, 7)], [env(e) for e in range(1, 7)])


# ---- LFO ----
def lfo(l):
    n = f"LFO {l} "
    return func(f"lfo_{l}", plain(
        vrow(spin("Sync", n + "Sync"), spin("Mode", n + "Sync Type"), spin("Tempo", n + "Tempo"), tog("Smooth", n + "Smooth Mode"), h=1.25),
        knobs(n, [("Rate", "Frequency"), ("Phase", "Phase"), ("Delay", "Delay"), ("Fade In", "Fade In")]),
        knobs(n, [("Stereo", "Stereo"), ("Smooth", "Smooth Time"), ("Transpose", "Transpose"), ("Tune", "Tune")]),
    ))


def rnd(r):
    n = f"Random LFO {r} "
    return func(f"random_{r}", plain(
        vrow(spin("Style", n + "Style"), spin("Sync", n + "Sync"), spin("Tempo", n + "Tempo"), tog("Sync Type", n + "Sync Type"), tog("Stereo", n + "Stereo"), h=1.25),
        knobs(n, [("Rate", "Frequency"), ("Transpose", "Transpose"), ("Tune", "Tune")]),
    ))


def lfo_tab():
    return listed(
        [f"LFO {l}" for l in range(1, 9)] + [f"RAND {r}" for r in range(1, 5)],
        [lfo(l) for l in range(1, 9)] + [rnd(r) for r in range(1, 5)],
    )


# ---- FX ----
def chorus():
    n = "Chorus "
    return func(f"chorus", plain(
        vrow(tog("On", n + "Switch"), spin("Sync", n + "Sync"), spin("Tempo", n + "Tempo"), h=1.25),
        knobs(n, [("Mix", "Mix"), ("Feedback", "Feedback"), ("Rate", "Frequency"), ("Depth", "Mod Depth")]),
        knobs(n, [("Delay 1", "Delay 1"), ("Delay 2", "Delay 2"), ("Voices", "Voices"), ("Cutoff", "Filter Cutoff"), ("Spread", "Filter Spread")]),
    ))


def compressor():
    n = "Compressor "
    rows = []
    for b in ("Low", "Band", "High"):
        rows.append(
            vrow(
                knob(f"{b} Up Thr", f"{b} Upper Threshold"),
                knob(f"{b} Up Ratio", f"{b} Upper Ratio"),
                knob(f"{b} Low Thr", f"{b} Lower Threshold"),
                knob(f"{b} Low Ratio", f"{b} Lower Ratio"),
            )
        )
    return func(f"compressor", plain(
        vrow(tog("On", n + "Switch"), spin("Bands", n + "Enabled Bands"), h=1.25),
        knobs(n, [("Attack", "Attack"), ("Release", "Release"), ("Mix", "Mix"), ("Low Gain", "Low Gain"), ("Band Gain", "Band Gain"), ("High Gain", "High Gain")]),
        *rows,
    ))


def delay():
    n = "Delay "
    return func(f"delay", plain(
        vrow(
            tog("On", n + "Switch"),
            spin("Style", n + "Style"),
            spin("Sync", n + "Sync"),
            spin("Tempo", n + "Tempo"),
            spin("Sync 2", n + "Sync 2"),
            spin("Tempo 2", n + "Tempo 2"),
            h=1.25,
        ),
        knobs(n, [("Mix", "Mix"), ("Feedback", "Feedback"), ("Time", "Frequency"), ("Time 2", "Frequency 2"), ("Cutoff", "Filter Cutoff"), ("Spread", "Filter Spread")]),
    ))


def distortion():
    n = "Distortion "
    return func(f"distortion", plain(
        vrow(tog("On", n + "Switch"), spin("Type", n + "Type"), spin("Filter Order", n + "Filter Order"), h=1.25),
        knobs(n, [("Drive", "Drive"), ("Mix", "Mix"), ("Cutoff", "Filter Cutoff"), ("Resonance", "Filter Resonance"), ("Blend", "Filter Blend")]),
    ))


def eq():
    n = "EQ "
    return func(f"eq", plain(
        vrow(tog("On", n + "Switch"), spin("Low", n + "Low Mode"), spin("Band", n + "Band Mode"), spin("High", n + "High Mode"), h=1.25),
        knobs(n, [("Low Cutoff", "Low Cutoff"), ("Low Gain", "Low Gain"), ("Low Res", "Low Resonance")]),
        knobs(n, [("Band Cutoff", "Band Cutoff"), ("Band Gain", "Band Gain"), ("Band Res", "Band Resonance")]),
        knobs(n, [("High Cutoff", "High Cutoff"), ("High Gain", "High Gain"), ("High Res", "High Resonance")]),
    ))


def flanger():
    n = "Flanger "
    return func(f"flanger", plain(
        vrow(tog("On", n + "Switch"), spin("Sync", n + "Sync"), spin("Tempo", n + "Tempo"), h=1.25),
        knobs(n, [("Mix", "Mix"), ("Feedback", "Feedback"), ("Rate", "Frequency"), ("Depth", "Mod Depth"), ("Phase", "Phase Offset"), ("Center", "Center")]),
    ))


def phaser():
    n = "Phaser "
    return func(f"phaser", plain(
        vrow(tog("On", n + "Switch"), spin("Sync", n + "Sync"), spin("Tempo", n + "Tempo"), h=1.25),
        knobs(n, [("Mix", "Mix"), ("Feedback", "Feedback"), ("Rate", "Frequency"), ("Depth", "Mod Depth")]),
        knobs(n, [("Phase", "Phase Offset"), ("Center", "Center"), ("Blend", "Blend")]),
    ))


def reverb():
    n = "Reverb "
    return func(f"reverb", plain(
        vrow(tog("On", n + "Switch"), h=1.25),
        knobs(n, [("Mix", "Mix"), ("Decay", "Decay Time"), ("Size", "Size"), ("Delay", "Delay")]),
        knobs(n, [("Pre Low", "Pre Low Cutoff"), ("Pre High", "Pre High Cutoff"), ("Low Cut", "Low Cutoff"), ("Low Gain", "Low Gain")]),
        knobs(n, [("High Cut", "High Cutoff"), ("High Gain", "High Gain"), ("Chorus", "Chorus Amount"), ("Chorus Rate", "Chorus Frequency")]),
    ))


def fx_tab():
    return listed(
        ["CHORUS", "COMP", "DELAY", "DISTORT", "EQ", "FILTER", "FLANGER", "PHASER", "REVERB"],
        [chorus(), compressor(), delay(), distortion(), eq(), filt("Filter fx"), flanger(), phaser(), reverb()],
    )


# ---- MOD ----
def slot(s):
    n = f"Modulation {s} "
    return vrow(
        spin("Source", n + "Source", 11).w_flex(2.2),
        spin("Destination", n + "Destination", 25).w_flex(2.8),
        knob("Amount", n + "Amount"),
        knob("Curve", n + "Power"),
        f"vstack([\n{tog('Bipolar', n + 'Bipolar')},\n{tog('Stereo', n + 'Stereo')},\n{tog('Bypass', n + 'Bypass')},\n])",
    )


def mod_tab():
    macros = vrow(*[knob(f"Macro {m}", f"Macro {m}") for m in range(1, 5)], h=2.3)
    lid = nid()
    pages = []
    for k in range(4):
        pages.append(func(f"modulation_{k + 1}", plain(*[slot(4 * k + i + 1) for i in range(4)])))
    labels = list_items(f"LIST_{lid}", ["MOD 1-4", "MOD 5-8", "MOD 9-12", "MOD 13-16"])
    parts = [f'vstack([list({lid}, {labels}, Bar::Right)]).w(Size::U(2.4))']
    for k, p in enumerate(pages):
        parts.append(f"{p}.show(Show::Setting({lid}, {k}))")
    return plain(macros, "hstack([\n" + ",\n".join(parts) + ",\n])")


# ---- VOICE ----
def voice_tab():
    return func(f"voice", plain(
        vrow(
            tog("Legato", "Legato"),
            tog("Port Force", "Portamento Force"),
            tog("Port Scale", "Portamento Scale"),
            spin("Oversampling", "Oversampling"),
            spin("Priority", "Voice Priority"),
            spin("Override", "Voice Override"),
            spin("Stereo Mode", "Stereo Mode"),
            h=1.25,
        ),
        vrow(*[knob(l, n) for l, n in [("Volume", "Volume"), ("Polyphony", "Polyphony"), ("Bend Range", "Pitch Bend Range"), ("Velocity", "Velocity Track"), ("Amplitude", "Voice Amplitude")]]),
        vrow(*[knob(l, n) for l, n in [("Transpose", "Voice Transpose"), ("Tune", "Voice Tune"), ("Stereo", "Stereo Routing"), ("Glide", "Portamento Time"), ("Glide Slope", "Portamento Slope")]]),
    ))


TABS = ["OSC", "FILTER", "ENV", "LFO", "FX", "MOD", "VOICE"]
pages = [osc_tab(), filter_tab(), env_tab(), lfo_tab(), fx_tab(), mod_tab(), voice_tab()]

# `P::ENABLE` has no name in the table's rows, so `NAMES` are the real params.
missing = [n for n in NAMES if n not in bound]
if missing:
    sys.exit(f"params without a control: {missing}")

shown = ",\n".join(f"{p}.show(Show::Tab({k}))" for k, p in enumerate(pages))
out = f'''//! The Vital panel (`honeydew_audio::vital`), UIHeight 12 U: Honeydew's own card, a tab strip over pages with the
//! sub-pages picked by param-less lists, one control for every parameter of Vital's engine and its 16 modulation
//! slots. Vital's own interface (Matt Tytel, GPL-3.0-or-later) is drawn with JUCE/OpenGL and cannot run on Android, so
//! this card stands in for it. Generated by `tools/gen_vital_panel.py`: do not edit.

use crate::panels::build::*;
use crate::params::VITAL_MODULE;

{chr(10).join(CONSTS)}
/// The index of the param named `name` in the module's table.
fn ix(name: &str) -> usize {{
    VITAL_MODULE
        .params
        .iter()
        .position(|p| p.name == name)
        .unwrap_or_else(|| panic!("Vital has no param {{name}}"))
}}

pub fn panel() -> Option<Panel> {{
    Some(Panel::new(vstack([
        tabs(1, "{" · ".join(TABS)}").h(Size::U(1.25)),
{shown},
    ])))
}}

{chr(10).join(FUNCS)}'''
open("src/panels/inst/vital.rs", "w").write(out)
print("controls:", len(bound), "ids:", next_id[0])
