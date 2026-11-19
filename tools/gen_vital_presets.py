#!/usr/bin/env python3
"""Generates `src/content/presets/vital.rs`: Honeydew's own Vital sounds, written in Vital's own units.

Vital ships no factory presets with its source, so these are ours (the sound design and the names). A value is the
engine's own number, which the script turns into the 0…1 the host stores by the range of `vendor/vital/tables`:

    0.5          the engine value as it is (a level 0…1, a semitone count, a menu index, a toggle)
    T(0.3)       seconds, for the envelope times (Vital stores their fourth root)
    HZ(2.0)      Hz, for a rate or a frequency (Vital stores its log2)
    SEC(1.5)     seconds, for the reverb and portamento times (Vital stores their log2)
    DB(-9)       dB, for Volume
    WT("Organ")  a wavetable by name
    SRC("lfo_1") / DST("filter_1_cutoff")  a matrix source / destination by Vital's name

Run from the repository root; with a path it also writes the presets as JSON for the engine-side render check:

    python3 tools/gen_vital_presets.py [presets.json]
"""
import json
import math
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLES = ROOT / "vendor/vital/tables"

WAVETABLES = [
    "Init Saw", "Basic Shapes", "Saw to Square", "Pulse Width", "Harmonic Sweep", "Odd Harmonics",
    "Vowels", "FM Bell", "Sine Fold", "Organ", "Sync Sweep", "Digital Steps",
]  # fmt: skip
SOURCES = (TABLES / "mod_sources.txt").read_text().split()
DESTINATIONS = [
    d
    for d in (TABLES / "mod_destinations.txt").read_text().split()
    if not d.startswith("modulation_") and d != "beats_per_minute"
]

RANGE = {}  # engine name -> (host label, min, max)
for line in (TABLES / "params.tsv").read_text().splitlines():
    f = line.split("\t")
    RANGE[f[1]] = (f[2].strip(), float(f[4]), float(f[5]))
for s in range(1, 17):
    RANGE[f"modulation_{s}_source"] = (f"Modulation {s} Source", 0.0, float(len(SOURCES)))
    RANGE[f"modulation_{s}_destination"] = (f"Modulation {s} Destination", 0.0, float(len(DESTINATIONS)))
for o in range(1, 4):
    RANGE[f"osc_{o}_wavetable"] = (f"Oscillator {o} Wavetable", 0.0, float(len(WAVETABLES) - 1))
RANGE["sub_waveform"] = ("Sub Osc Waveform", 0.0, 5.0)


class T(float): ...


class HZ(float): ...


class SEC(float): ...


class DB(float): ...


class WT(str): ...


class SRC(str): ...


class DST(str): ...


def engine_value(name, v):
    if isinstance(v, T):
        return float(v) ** 0.25
    if isinstance(v, (HZ, SEC)):
        return math.log2(float(v))
    if isinstance(v, DB):
        return (float(v) + 80.0) ** 2
    if isinstance(v, WT):
        return float(WAVETABLES.index(v))
    if isinstance(v, SRC):
        return float(1 + SOURCES.index(v))
    if isinstance(v, DST):
        return float(1 + DESTINATIONS.index(v))
    return float(v)


def normalised(name, v):
    if name not in RANGE:
        sys.exit(f"no Vital param {name!r}")
    label, lo, hi = RANGE[name]
    x = (engine_value(name, v) - lo) / (hi - lo)
    if not -1e-6 <= x <= 1 + 1e-6:
        sys.exit(f"{name} = {v!r} is out of its range {lo}…{hi}")
    return label, min(1.0, max(0.0, x))


def slot(n, source, destination, amount, power=0.0, bipolar=0, stereo=0):
    return {
        f"modulation_{n}_source": SRC(source),
        f"modulation_{n}_destination": DST(destination),
        f"modulation_{n}_amount": amount,
        f"modulation_{n}_power": power,
        f"modulation_{n}_bipolar": bipolar,
        f"modulation_{n}_stereo": stereo,
    }


def env(n, attack=0.002, decay=0.4, sustain=0.7, release=0.25, hold=0.0, delay=0.0):
    return {
        f"env_{n}_attack": T(attack),
        f"env_{n}_decay": T(decay),
        f"env_{n}_sustain": sustain,
        f"env_{n}_release": T(release),
        f"env_{n}_hold": T(hold),
        f"env_{n}_delay": T(delay),
    }


def osc(n, table, frame=0, level=0.8, voices=1, detune=2.5, transpose=0, tune=0.0, pan=0.0, blend=0.8, **more):
    d = {
        f"osc_{n}_on": 1,
        f"osc_{n}_wavetable": WT(table),
        f"osc_{n}_wave_frame": frame,
        f"osc_{n}_level": level,
        f"osc_{n}_unison_voices": voices,
        f"osc_{n}_unison_detune": detune,
        f"osc_{n}_transpose": transpose,
        f"osc_{n}_tune": tune,
        f"osc_{n}_pan": pan,
        f"osc_{n}_unison_blend": blend,
    }
    d.update({f"osc_{n}_{k}": v for k, v in more.items()})
    return d


def filt(n, model=0, style=1, cutoff=80, resonance=0.3, drive=0.0, keytrack=0.0, **more):
    d = {
        f"filter_{n}_on": 1,
        f"filter_{n}_model": model,
        f"filter_{n}_style": style,
        f"filter_{n}_cutoff": cutoff,
        f"filter_{n}_resonance": resonance,
        f"filter_{n}_drive": drive,
        f"filter_{n}_keytrack": keytrack,
        f"filter_{n}_osc1_input": 1,
        f"filter_{n}_osc2_input": 1,
        f"filter_{n}_osc3_input": 1,
        f"filter_{n}_sample_input": 1,
    }
    d.update({f"filter_{n}_{k}": v for k, v in more.items()})
    return d


def lfo(n, rate=2.0, sync_type=0, **more):
    d = {f"lfo_{n}_sync": 0, f"lfo_{n}_frequency": HZ(rate), f"lfo_{n}_sync_type": sync_type}
    d.update({f"lfo_{n}_{k}": v for k, v in more.items()})
    return d


def fx(name, **values):
    d = {f"{name}_on": 1}
    d.update({f"{name}_{k}": v for k, v in values.items()})
    return d


def merge(*parts):
    out = {}
    for p in parts:
        out.update(p)
    return out


# (category, name, params). Anything not named keeps Vital's own default (an init saw on Oscillator 1).
PRESETS = [
    # ---- Bass
    ("Bass", "Sub Pressure", merge(
        osc(1, "Init Saw", level=0.7, transpose=-12),
        {"sub_on": 1, "sub_waveform": 0, "sub_level": 0.85, "sub_transpose": -12},
        filt(1, cutoff=52, resonance=0.25, keytrack=0.3),
        env(1, 0.002, 0.3, 0.9, 0.12),
        env(2, 0.001, 0.22, 0.15, 0.12),
        slot(1, "env_2", "filter_1_cutoff", 0.28),
        {"velocity_track": 0.35, "polyphony": 4, "volume": DB(-2.5)},
    )),
    ("Bass", "Reese Drift", merge(
        osc(1, "Init Saw", level=0.55, voices=2, detune=1.6, transpose=-12),
        osc(2, "Init Saw", level=0.55, voices=2, detune=1.6, transpose=-12, tune=0.08),
        filt(1, cutoff=62, resonance=0.3, keytrack=0.4),
        lfo(1, 0.22),
        fx("chorus", dry_wet=0.35, frequency=HZ(0.4)),
        env(1, 0.004, 0.5, 0.9, 0.2),
        slot(1, "lfo_1", "filter_1_cutoff", 0.1, bipolar=1),
        {"polyphony": 4, "volume": DB(-10.5)},
    )),
    ("Bass", "Wobble Square", merge(
        osc(1, "Saw to Square", frame=150, level=0.8, transpose=-12),
        {"sub_on": 1, "sub_waveform": 3, "sub_level": 0.5, "sub_transpose": -12},
        filt(1, cutoff=66, resonance=0.55, drive=3.0),
        lfo(1, 3.0),
        env(1, 0.002, 0.3, 1.0, 0.1),
        slot(1, "lfo_1", "filter_1_cutoff", 0.32, bipolar=1),
        {"polyphony": 2, "volume": DB(-5)},
    )),
    ("Bass", "808 Boom", merge(
        osc(1, "Basic Shapes", frame=0, level=0.95, transpose=-24),
        env(1, 0.001, 1.2, 0.0, 0.5),
        env(2, 0.001, 0.07, 0.0, 0.07),
        slot(1, "env_2", "osc_1_transpose", 0.13),
        {"polyphony": 1, "volume": DB(-10.5)},
    )),
    # ---- Lead
    ("Lead", "Supersaw Lead", merge(
        osc(1, "Init Saw", level=0.6, voices=7, detune=3.0, blend=0.9),
        osc(2, "Init Saw", level=0.45, voices=5, detune=2.2, transpose=12),
        filt(1, cutoff=104, resonance=0.2),
        env(1, 0.004, 0.5, 0.85, 0.3),
        fx("delay", dry_wet=0.2, feedback=0.4),
        fx("reverb", dry_wet=0.2, decay_time=SEC(1.8)),
        slot(1, "mod_wheel", "filter_1_cutoff", 0.15),
        {"polyphony": 6, "volume": DB(-7.5)},
    )),
    ("Lead", "Bell Lead", merge(
        osc(1, "FM Bell", frame=40, level=0.8),
        env(1, 0.002, 1.6, 0.0, 1.0),
        env(2, 0.002, 0.9, 0.0, 0.6),
        slot(1, "env_2", "osc_1_wave_frame", 0.3),
        fx("reverb", dry_wet=0.3, decay_time=SEC(2.8)),
        {"volume": DB(-9.5)},
    )),
    ("Lead", "Sync Scream", merge(
        osc(1, "Sync Sweep", frame=80, level=0.7, voices=3, detune=1.4),
        filt(1, cutoff=112, resonance=0.25),
        env(1, 0.003, 0.4, 0.9, 0.2),
        lfo(1, 5.2, delay_time=0.3),
        slot(1, "lfo_1", "osc_1_wave_frame", 0.18, bipolar=1),
        slot(2, "mod_wheel", "osc_1_wave_frame", 0.4),
        fx("delay", dry_wet=0.18, feedback=0.35),
        {"portamento_time": SEC(0.08), "polyphony": 1, "volume": DB(-5)},
    )),
    ("Lead", "Pulse Chorus", merge(
        osc(1, "Pulse Width", frame=90, level=0.75, voices=2, detune=1.2),
        filt(1, cutoff=96, resonance=0.15),
        env(1, 0.004, 0.4, 0.9, 0.25),
        lfo(1, 0.8),
        slot(1, "lfo_1", "osc_1_wave_frame", 0.2, bipolar=1),
        fx("chorus", dry_wet=0.4),
        fx("reverb", dry_wet=0.15),
        {"volume": DB(-10.5)},
    )),
    # ---- Pad
    ("Pad", "Glass Pad", merge(
        osc(1, "Harmonic Sweep", frame=60, level=0.7, voices=5, detune=2.4),
        filt(1, cutoff=100, resonance=0.15),
        env(1, 0.55, 1.0, 0.9, 1.4),
        lfo(1, 0.12),
        slot(1, "lfo_1", "osc_1_wave_frame", 0.22, bipolar=1),
        fx("chorus", dry_wet=0.4),
        fx("reverb", dry_wet=0.35, decay_time=SEC(3.5)),
        {"volume": DB(-6.5)},
    )),
    ("Pad", "Warm Strings", merge(
        osc(1, "Init Saw", level=0.55, voices=6, detune=2.2),
        osc(2, "Init Saw", level=0.4, voices=4, detune=1.8, transpose=-12),
        filt(1, cutoff=84, resonance=0.1, keytrack=0.4),
        env(1, 0.35, 1.0, 0.9, 0.9),
        fx("chorus", dry_wet=0.5, frequency=HZ(0.35)),
        fx("reverb", dry_wet=0.3, decay_time=SEC(2.6)),
        {"volume": DB(-10.5)},
    )),
    ("Pad", "Vowel Choir", merge(
        osc(1, "Vowels", frame=20, level=0.7, voices=3, detune=1.2),
        env(1, 0.4, 1.0, 0.9, 1.1),
        lfo(1, 0.18),
        slot(1, "lfo_1", "osc_1_wave_frame", 0.35, bipolar=1),
        fx("reverb", dry_wet=0.4, decay_time=SEC(3.2)),
        {"volume": DB(-10.5)},
    )),
    ("Pad", "Organ Drift", merge(
        osc(1, "Organ", frame=30, level=0.7, voices=2, detune=0.8),
        env(1, 0.05, 0.6, 1.0, 0.5),
        fx("phaser", dry_wet=0.5, frequency=HZ(0.25), feedback=0.4),
        fx("chorus", dry_wet=0.3),
        {"volume": DB(-10)},
    )),
    # ---- Pluck
    ("Pluck", "Soft Pluck", merge(
        osc(1, "Basic Shapes", frame=70, level=0.8, voices=2, detune=0.8),
        filt(1, cutoff=62, resonance=0.2, keytrack=0.4),
        env(1, 0.001, 0.35, 0.0, 0.3),
        env(2, 0.001, 0.25, 0.0, 0.2),
        slot(1, "env_2", "filter_1_cutoff", 0.32),
        fx("reverb", dry_wet=0.2, decay_time=SEC(1.4)),
        {"volume": DB(-3.5)},
    )),
    ("Pluck", "Digital Stab", merge(
        osc(1, "Digital Steps", frame=100, level=0.75, voices=3, detune=1.6),
        env(1, 0.001, 0.2, 0.0, 0.18),
        env(2, 0.001, 0.15, 0.0, 0.1),
        slot(1, "env_2", "osc_1_wave_frame", 0.4),
        fx("delay", dry_wet=0.25, feedback=0.4),
        {"volume": DB(-7.5)},
    )),
    ("Pluck", "Fold Marimba", merge(
        osc(1, "Sine Fold", frame=10, level=0.85),
        env(1, 0.001, 0.6, 0.0, 0.5),
        env(2, 0.001, 0.18, 0.0, 0.12),
        slot(1, "env_2", "osc_1_wave_frame", 0.35),
        fx("reverb", dry_wet=0.25, decay_time=SEC(1.8)),
        {"volume": DB(-8.5)},
    )),
    # ---- Keys
    ("Keys", "Glass Keys", merge(
        osc(1, "FM Bell", frame=20, level=0.7, voices=2, detune=0.6),
        env(1, 0.002, 1.8, 0.1, 0.6),
        env(2, 0.002, 0.5, 0.0, 0.4),
        slot(1, "env_2", "osc_1_wave_frame", 0.2),
        slot(2, "velocity", "osc_1_wave_frame", 0.15),
        fx("chorus", dry_wet=0.25),
        fx("reverb", dry_wet=0.25, decay_time=SEC(2.2)),
        {"volume": DB(-9)},
    )),
    ("Keys", "Hollow Keys", merge(
        osc(1, "Odd Harmonics", frame=50, level=0.75),
        filt(1, cutoff=84, resonance=0.1, keytrack=0.5),
        env(1, 0.002, 1.2, 0.2, 0.5),
        slot(1, "velocity", "filter_1_cutoff", 0.2),
        fx("chorus", dry_wet=0.3, frequency=HZ(0.5)),
        fx("reverb", dry_wet=0.2),
        {"volume": DB(-7.5)},
    )),
    # ---- FX
    ("FX", "Wind Chime Haze", merge(
        osc(1, "FM Bell", frame=120, level=0.6, voices=6, detune=5.5),
        env(1, 1.0, 1.5, 0.8, 2.0),
        {"random_1_frequency": HZ(0.4), "random_1_style": 0},
        slot(1, "random_1", "osc_1_wave_frame", 0.5, bipolar=1),
        fx("reverb", dry_wet=0.5, decay_time=SEC(5.0)),
        fx("delay", dry_wet=0.3, feedback=0.55),
        {"volume": DB(-5.5)},
    )),
    ("FX", "Distant Engine", merge(
        osc(1, "Init Saw", level=0.7, voices=9, detune=6.0, transpose=-24),
        filt(1, cutoff=48, resonance=0.4),
        env(1, 0.8, 1.0, 1.0, 1.2),
        lfo(1, 0.15),
        slot(1, "lfo_1", "filter_1_cutoff", 0.16, bipolar=1),
        fx("distortion", drive=8.0, mix=0.4),
        {"volume": DB(-6.5)},
    )),
]


def rust_str(s):
    return '"' + s.replace('"', '\\"') + '"'


def main():
    out = [
        "//! The Vital bank, `Presets/Vital/<Category>/<Name>.flmpst`: Honeydew's own sounds. Vital ships none with its source,",
        "//! so these are ours, written in Vital's units by `tools/gen_vital_presets.py` (do not edit this file): each a set of",
        "//! the module's params by name, normalised over Vital's own ranges; the rest keep Vital's defaults.",
        "",
        "use super::{Preset, p};",
        "",
        "/// The Vital bank: a module type and its presets.",
        "pub(super) const BANKS: &[(u16, &[Preset])] = &[(honeydew_audio::vital::VITAL, BANK)];",
        "",
        "const BANK: &[Preset] = &[",
    ]
    names = set()
    dump = []
    for category, name, params in PRESETS:
        if (category, name) in names:
            sys.exit(f"{name} twice")
        names.add((category, name))
        rows = []
        for key, v in params.items():
            label, x = normalised(key, v)
            rows.append((label, round(x, 6), key))
        if len({r[0] for r in rows}) != len(rows):
            sys.exit(f"{name}: a param twice")
        dump.append(dict(category=category, name=name, params=[[k, x] for _, x, k in rows]))
        out.append(f"    p({rust_str(category)}, {rust_str(name)}, &[")
        for label, x, _ in rows:
            out.append(f"        ({rust_str(label)}, {x!r}),")
        out.append("    ]),")
    out.append("];")
    path = ROOT / "src/content/presets/vital.rs"
    path.write_text("\n".join(out) + "\n")
    subprocess.run(["rustfmt", "--edition", "2024", str(path)], check=True)
    if len(sys.argv) > 1:
        Path(sys.argv[1]).write_text(json.dumps(dump))
    print(len(PRESETS), "presets")


main()
