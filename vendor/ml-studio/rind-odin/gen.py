#!/usr/bin/env python3
"""Writes src/settings.rs and banks/patches.bin for Rind Odin.

usage: gen.py <Odin2>/assets

The settings are Odin 2's 258 parameters (banks/params.tsv: id, name, kind, min, max, skew, interval, default) and the
properties of a patch that are not parameters but decide the sound (oscillator and filter types, waveforms, the mod
matrix, ...). banks/patches.bin holds the factory patches as Odin saves them, so every one loads exactly."""
import os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
assets = sys.argv[1]

# --- tables (value written to the patch, name) ---------------------------------------------------------------------
OSC_TYPES = [(1, "OFF"), (2, "ANALOG"), (3, "WAVETABLE"), (4, "MULTI"), (5, "VECTOR"), (6, "CHIPTUNE"), (7, "FM"), (8, "PM"),
             (15, "NOISE"), (20, "WAVEDRAW"), (21, "CHIPDRAW"), (22, "SPECDRAW")]
FIL_TYPES = [(1, "NONE"), (2, "LP 24"), (3, "LP 12"), (4, "BP 24"), (5, "BP 12"), (6, "HP 24"), (7, "HP 12"), (15, "SEM 12"),
             (19, "DIODE"), (24, "KORG LP"), (25, "KORG HP"), (30, "COMB"), (35, "FORMANT"), (40, "RING MOD")]
SOURCES = [(0, "Off"), (100, "Oscillator 1"), (101, "Oscillator 2"), (102, "Oscillator 3"), (110, "Filter 1 Out"),
           (111, "Filter 2 Out"), (200, "Amp Envelope"), (201, "Filter Envelope"), (202, "Mod Envelope"),
           (203, "Global Envelope"), (300, "LFO 1"), (301, "LFO 2"), (302, "LFO 3"), (303, "Global LFO"), (400, "X"),
           (401, "Y"), (402, "ModWheel"), (403, "PitchBend"), (404, "MIDI Note"), (405, "MIDI Velocity"),
           (407, "MIDI Breath"), (409, "Channel Pressure"), (420, "Unison Index"), (430, "Arp Mod 1"), (435, "Arp Mod 2"),
           (440, "Sustain Pedal"), (450, "Soft Pedal"), (999, "Random"), (1000, "Constant")]
ARP_DIR = [(10, "UP"), (20, "DOWN"), (30, "UP DOWN"), (40, "DOWN UP"), (50, "RANDOM"), (60, "CRAWL UP"), (70, "CRAWL DOWN"),
           (80, "CRAWL UD"), (90, "CRAWL DU")]
LEGATO = [(0, "LEGATO"), (1, "POLY"), (2, "RETRIG")]
DIST_ALGO = [(1, "ALGO 1"), (2, "ALGO 2"), (3, "ALGO 3")]
ANALOG = [(0, "SAW"), (1, "PULSE"), (2, "TRIANGLE"), (3, "SINE")]
LFO_SHAPE = [(i, "S&H" if i == 7 else f"SHAPE {i + 1 if i < 7 else i}") for i in range(14)]
DENOM = [(i, str(d)) for i, d in enumerate([1, 2, 4, 8, 12, 16, 24, 32, 48])]
WAVETABLES = []
for line in open(os.path.join(HERE, "banks", "wavetables.tsv")):
    i, name = line.rstrip("\n").split("\t")
    WAVETABLES.append((int(i), name))

DESTS = [(0, "Off", "OFF")]
for o in range(3):
    for code, name in [(2, "Pitch Exp"), (3, "Pitch Lin"), (4, "Volume"), (10, "Pulse Width"), (20, "Position"), (21, "Detune"),
                       (22, "Spread"), (30, "X"), (31, "Y"), (40, "Arp Speed"), (50, "FM/PM Amount"), (51, "Carrier Ratio"),
                       (52, "Modulator Ratio"), (60, "LP Frequency"), (61, "HP Frequency")]:
        DESTS.append((100 * o + code, f"Osc{o + 1} {name}", f"OSC {o + 1}"))
for f in range(3):
    for code, name in [(1, "Frequency"), (2, "Resonance"), (3, "Gain"), (4, "Env Amount"), (5, "Vel Amount"), (6, "Kbd Amount"),
                       (7, "Saturation"), (10, "SEM Transition"), (20, "Formant Transition"), (30, "RingMod Amount")]:
        DESTS.append((300 + 100 * f + code, f"Filter{f + 1} {name}", f"FILTER {f + 1}"))
for m, en in enumerate(["Amp Env", "Filter Env", "Mod Env", "Global Env"]):
    for code, name in [(1, "Attack"), (2, "Decay"), (3, "Sustain"), (4, "Release")]:
        DESTS.append((600 + 10 * m + code, f"{en} {name}", "ENVELOPE"))
for m in range(4):
    DESTS.append((650 + 10 * m + 1, f"LFO{m + 1} Freq" if m < 3 else "Global LFO Freq", "LFO"))
for base, pre, names in [(700, "Delay", ["Time", "Feedback", "HP Freq", "Dry", "Wet"]),
                         (750, "Phaser", ["Rate", "Amount", "Freq", "Feedback", "DryWet"]),
                         (800, "Flanger", ["Rate", "Amount", "Feedback", "DryWet"]),
                         (850, "Chorus", ["Rate", "Amount", "Feedback", "DryWet"])]:
    for i, n in enumerate(names):
        DESTS.append((base + 1 + i, f"{pre} {n}", "EFFECTS"))
DESTS += [(900, "Amp Gain", "AMP"), (901, "Amp Pan", "AMP"), (950, "Distortion Boost", "EFFECTS"),
          (951, "Distortion DryWet", "EFFECTS"), (970, "Arp Speed", "ARP"), (973, "Arp Gate", "ARP"),
          (980, "XY-Pad X", "XY"), (982, "XY-Pad Y", "XY")]
DEST_GROUPS = []
for _, _, g in DESTS:
    if g not in DEST_GROUPS:
        DEST_GROUPS.append(g)
DEST_GROUP_OF = [DEST_GROUPS.index(g) for _, _, g in DESTS]
DESTS = [(i, n) for i, n, _ in DESTS]

TABLES = [("OSC_TYPES", OSC_TYPES), ("FIL_TYPES", FIL_TYPES), ("SOURCES", SOURCES), ("DESTS", DESTS), ("ARP_DIR", ARP_DIR),
          ("LEGATO", LEGATO), ("DIST_ALGO", DIST_ALGO), ("ANALOG", ANALOG), ("LFO_SHAPE", LFO_SHAPE),
          ("WAVETABLES", WAVETABLES), ("DENOM", DENOM),
          # The voice counts the engine has pan and gain tables for: any other number reads past them.
          ("UNISON", [(1, "1 voice"), (2, "2 voices"), (3, "3 voices"), (4, "4 voices"), (6, "6 voices"), (12, "12 voices")])]
T = {n: i for i, (n, _) in enumerate(TABLES)}

# --- settings ----------------------------------------------------------------------------------------------------------
# (id, name, tree, key, kind, lo, hi, skew, default, unit, table)
# kind: f float parameter, i int parameter, n property chosen from a table, q int property, a float property in -1..1
S = []


def unit_of(id, kind, lo, hi):
    if kind == "i":
        if hi - lo == 1 and lo == 0:
            return "b"
        if id.endswith("_oct"):
            return "o"
        if id.endswith("_semi") or "_step_" in id and not id.endswith("_on") or id.endswith("_transpose"):
            return "t"
        return "i"
    if id.endswith("_fine"):
        return "c"
    if id in ("amp_gain", "master") or id.endswith("_gain") and id.startswith("fil") or id.endswith("_vol"):
        return "d"
    if re.search(r"(attack|decay|release)$", id) or id == "delay_time":
        return "s"
    if re.search(r"(_freq|_lp|_hp|eqfreq|hf_damp)$", id) and hi >= 100:
        return "h"
    if id.startswith(("lfo", "chorus_rate", "flanger_rate", "phaser_rate")):
        return "h"
    if lo == 0 and hi == 1:
        return "%"
    if lo == -1 and hi == 1:
        return "x"
    return "p"


for line in open(os.path.join(HERE, "banks", "params.tsv")):
    f = line.rstrip("\n").split("\t")
    id, name, kind, lo, hi, skew, interval, default = f[0], f[1], f[2], float(f[3]), float(f[4]), float(f[5]), float(f[6]), float(f[7])
    S.append((id, name, "", id, kind, lo, hi, skew, default, unit_of(id, kind, lo, hi), -1))


def prop(id, name, tree, key, kind, lo, hi, default, unit, table=-1):
    S.append((id, name, tree, key, kind, lo, hi, 1.0, default, unit, table))


def idx(table, value):
    for i, (v, _) in enumerate(TABLES[T[table]][1]):
        if v == value:
            return i
    raise KeyError((table, value))


for n in (1, 2, 3):
    prop(f"osc{n}_type", f"Osc {n} Type", "osc", f"osc{n}_type", "n", 0, len(OSC_TYPES) - 1, idx("OSC_TYPES", 2 if n == 1 else 1), "n", T["OSC_TYPES"])
    prop(f"osc{n}_analog_wave", f"Osc {n} Wave", "osc", f"osc{n}_analog_wave", "n", 0, 3, 0, "n", T["ANALOG"])
    prop(f"osc{n}_wavetable", f"Osc {n} Wavetable", "osc", f"osc{n}_wavetable", "n", 0, len(WAVETABLES) - 1, 0, "n", T["WAVETABLES"])
for n in (1, 2, 3):
    prop(f"fil{n}_type", f"Filter {n} Type", "misc", f"fil{n}_type", "n", 0, len(FIL_TYPES) - 1, idx("FIL_TYPES", 2 if n == 1 else 1), "n", T["FIL_TYPES"])
prop("legato", "Voice Mode", "misc", "legato", "n", 0, 2, 1, "n", T["LEGATO"])
prop("dist_on", "Distortion On", "misc", "dist_on", "q", 0, 1, 0, "b")
prop("dist_algo", "Distortion Algorithm", "misc", "dist_algo", "n", 0, 2, 0, "n", T["DIST_ALGO"])
prop("pitchbend_amount", "Pitchbend Range", "misc", "pitchbend_amount", "q", 0, 24, 12, "t")
prop("unison_voices", "Unison Voices", "misc", "unison_voices", "n", 0, 5, 0, "n", T["UNISON"])
prop("arp_octaves", "Arp Octaves", "misc", "arp_octaves", "q", 0, 3, 2, "i")
prop("arp_direction", "Arp Direction", "misc", "arp_direction", "n", 0, len(ARP_DIR) - 1, 0, "n", T["ARP_DIR"])
prop("arp_steps", "Arp Steps", "misc", "arp_steps", "q", 1, 16, 16, "i")
prop("arp_gate", "Arp Gate", "misc", "arp_gate", "q", 1, 100, 50, "i")
for n in (1, 2, 3, 4):
    prop(f"lfo{n}_wave", f"LFO {n} Shape", "lfo", f"lfo{n}_wave", "n", 0, 13, 0, "n", T["LFO_SHAPE"])
for n in (1, 2, 3, 4):
    prop(f"lfo{n}_sync", f"LFO {n} Sync", "lfo", f"lfo{n}_sync", "q", 0, 1, 0, "b")
prop("arp_synctime_numerator", "Arp Rate Numerator", "misc", "arp_synctime_numerator", "q", 0, 15, 1, "u")
prop("arp_synctime_denominator", "Arp Rate Denominator", "misc", "arp_synctime_denominator", "n", 0, len(DENOM) - 1, 5, "n", T["DENOM"])
for n in (1, 2, 3, 4):
    prop(f"lfo{n}_synctime_numerator", f"LFO {n} Rate Numerator", "lfo", f"lfo{n}_synctime_numerator", "q", 0, 15, 1, "u")
    prop(f"lfo{n}_synctime_denominator", f"LFO {n} Rate Denominator", "lfo", f"lfo{n}_synctime_denominator", "n", 0, len(DENOM) - 1, 5, "n", T["DENOM"])
for r in range(9):
    prop(f"mod_source_{r}", f"Mod {r + 1} Source", "mod", f"source_row_{r}", "n", 0, len(SOURCES) - 1, 0, "n", T["SOURCES"])
    prop(f"mod_dest1_{r}", f"Mod {r + 1} Dest 1", "mod", f"dest_1_row_{r}", "n", 0, len(DESTS) - 1, 0, "n", T["DESTS"])
    prop(f"mod_amount1_{r}", f"Mod {r + 1} Amount 1", "mod", f"amount_0_row_{r}", "a", -1000, 1000, 0, "x")
    prop(f"mod_dest2_{r}", f"Mod {r + 1} Dest 2", "mod", f"dest_2_row_{r}", "n", 0, len(DESTS) - 1, 0, "n", T["DESTS"])
    prop(f"mod_amount2_{r}", f"Mod {r + 1} Amount 2", "mod", f"amount_1_row_{r}", "a", -1000, 1000, 0, "x")
    prop(f"mod_scale_{r}", f"Mod {r + 1} Scale", "mod", f"scale_row_{r}", "n", 0, len(SOURCES) - 1, 0, "n", T["SOURCES"])
    prop(f"mod_scale_amount_{r}", f"Mod {r + 1} Scale Amount", "mod", f"amount_2_row_{r}", "a", -1000, 1000, 0, "x")

COUNT = len(S)
assert COUNT <= 384, COUNT


def norm(lo, hi, skew, real):
    p = (real - lo) / (hi - lo)
    p = min(max(p, 0.0), 1.0)
    return p ** skew if skew != 1.0 and p > 0 else p


def start_of(s):
    id, name, tree, key, kind, lo, hi, skew, default, unit, table = s
    if kind == "f":
        return round(norm(lo, hi, skew, default) * 1000)
    return round(default)


def const(id):
    return re.sub(r"[^A-Za-z0-9]", "_", id).upper()


def fl(x):
    t = f"{x:.9g}"
    return t if any(c in t for c in ".en") else t + ".0"


def rs(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


out = []
w = out.append
w("// SPDX-License-Identifier: GPL-3.0-or-later")
w("// Generated by gen.py from banks/params.tsv and Odin 2's mod matrix: do not edit.")
w("use ml_small_fx::Setting;")
w("")
w(f"pub const COUNT: usize = {COUNT};")
w("")
w("/// What a setting is in the engine.")
w("#[derive(Clone, Copy, PartialEq, Eq)]")
w("pub enum Kind {")
w("    /// A float parameter: the setting holds its place in the range (skewed) in thousandths.")
w("    Float,")
w("    /// An int parameter: the setting holds it.")
w("    Int,")
w("    /// A property picked from a table: the setting holds the place in the table.")
w("    Pick,")
w("    /// An int property: the setting holds it.")
w("    Prop,")
w("    /// A float property in -1..1: the setting holds it in thousandths.")
w("    Amount,")
w("}")
w("")
w("pub struct Info {")
w("    /// The parameter's id, or the property's key in its tree.")
w("    pub key: &'static str,")
w("    /// The tree a property is in; empty for a parameter.")
w("    pub tree: &'static str,")
w("    pub kind: Kind,")
w("    /// The engine's range of a float parameter.")
w("    pub lo: f32,")
w("    pub hi: f32,")
w("    pub skew: f32,")
w("    /// How the value reads: h Hz, d dB, s seconds, c cents, t semitones, o octaves, % percent, x signed percent,")
w("    /// b on/off, i whole number, u whole number from 1, p plain, n a table's name.")
w("    pub unit: u8,")
w("    /// The table of a `Pick`.")
w("    pub table: u8,")
w("}")
w("")
w("pub static SETTINGS: [Setting; COUNT] = [")
for s in S:
    id, name, tree, key, kind, lo, hi, skew, default, unit, table = s
    rng = (0, 1000) if kind == "f" else (int(lo), int(hi))
    st = start_of(s)
    w(f"    Setting {{ id: {rs(id)}, name: {rs(name)}, range: ({rng[0]}, {rng[1]}), start: {st}, magnet: None }},")
w("];")
w("")
w("pub static INFO: [Info; COUNT] = [")
KIND = {"f": "Float", "i": "Int", "n": "Pick", "q": "Prop", "a": "Amount"}
for s in S:
    id, name, tree, key, kind, lo, hi, skew, default, unit, table = s
    w(f"    Info {{ key: {rs(key)}, tree: {rs(tree)}, kind: Kind::{KIND[kind]}, lo: {fl(lo)}, hi: {fl(hi)}, skew: {fl(skew)}, "
      f"unit: b'{unit}', table: {max(table, 0)} }},")
w("];")
w("")
w("/// The place of every setting by its id.")
fam = {}
for k, s in enumerate(S):
    id = s[0]
    m = re.match(r"^(osc|fil|env|lfo)(\d)_(.+)$", id)
    m2 = re.match(r"^(step)_(\d+)_(.+)$", id)
    m3 = re.match(r"^(mod_.+?)_(\d)$", id)
    if m:
        fam.setdefault(f"{m.group(1)}_{m.group(3)}", {})[int(m.group(2))] = k
    elif m2:
        fam.setdefault(f"step_{m2.group(3)}", {})[int(m2.group(2))] = k
    elif m3:
        fam.setdefault(m3.group(1), {})[int(m3.group(2))] = k
    else:
        w(f"pub const {const(id)}: usize = {k};")
for name, d in sorted(fam.items()):
    lo = min(d)
    n = max(d) - lo + 1
    if sorted(d) == list(range(lo, lo + n)) and lo in (0, 1):
        w(f"pub const {const(name)}: [usize; {n}] = [{', '.join(str(d[i]) for i in range(lo, lo + n))}];")
    else:
        # a gap (the third filter has no second filter to feed from): only the first index of each is named
        for i, k in sorted(d.items()):
            w(f"pub const {const(name)}_{i}: usize = {k};")
w("")
w("/// Tables the properties are picked from: (the value in the patch, the name).")
for name, t in TABLES:
    w(f"pub static T_{name}: [(i32, &str); {len(t)}] = [")
    for v, n in t:
        w(f"    ({v}, {rs(n)}),")
    w("];")
w("")
w("/// The group of each destination (a tap on a destination's name jumps to the next group).")
w(f"pub static DEST_GROUP: [u8; {len(DESTS)}] = [{', '.join(map(str, DEST_GROUP_OF))}];")
w("pub const PICK: [&[(i32, &str)]; " + str(len(TABLES)) + "] = [" + ", ".join(f"&T_{n}" for n, _ in TABLES) + "];")
open(os.path.join(HERE, "src", "settings.rs"), "w").write("\n".join(out) + "\n")

# --- the patches -----------------------------------------------------------------------------------------------------
root = os.path.join(assets, "Soundbanks", "Factory Presets")
cats = sorted(os.listdir(root))
blob = bytearray()
patches = []
for ci, c in enumerate(cats):
    for f in sorted(os.listdir(os.path.join(root, c)), key=str.lower):
        if f.endswith(".odin"):
            patches.append((ci, os.path.splitext(f)[0], open(os.path.join(root, c, f), "rb").read()))
blob += struct.pack("<HH", len(cats), len(patches))
for c in cats:
    b = c.encode()
    blob += bytes([len(b)]) + b
for ci, name, data in patches:
    b = name.encode()
    blob += bytes([ci, len(b)]) + b + struct.pack("<I", len(data)) + data
open(os.path.join(HERE, "banks", "patches.bin"), "wb").write(blob)
print(COUNT, "settings,", len(patches), "patches,", len(blob), "bytes")
