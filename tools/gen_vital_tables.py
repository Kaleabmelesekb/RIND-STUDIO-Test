#!/usr/bin/env python3
"""Writes the two tables that describe Vital's parameters to the studio, from what Vital's own engine reports.

Input (vendor/vital/tables, written from the running engine by `tools/vital_tables_dump.sh`):
  params.tsv            Vital's `Parameters` list (index, name, display name, units, min, max, default, scale, offset,
                        multiplier, invert, version, menu items…)
  mod_sources.txt       the engine's modulation sources
  mod_destinations.txt  the engine's modulation destinations

Output:
  crates/audio/src/vital_table.rs   what the engine side needs: for each host param, which Vital param it is and its
                                    range (so a normalised 0…1 value becomes Vital's value), and the matrix's names
  src/params/vital_table.rs         the registry side: names, defaults and display laws

Run from the repository root; `cargo fmt` afterwards."""

import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLES = ROOT / "vendor/vital/tables"

SLOTS = 16  # modulation slots the card shows (Vital has 64)
WAVETABLES = 3  # one per oscillator

# Vital's parameters the studio does not show: internal, driven by the host, or by the plugin's own window.
EXCLUDED = {
    "effect_chain_order",  # the order of the effects: Vital's window drags them
    "compressor_low_band_unused",
    "view_spectrogram",
    "bypass",
    "mpe_enabled",
    "mod_wheel",  # the host's controllers
    "pitch_wheel",
    "beats_per_minute",  # the song's tempo
}

SUB_WAVEFORMS = ["Sine", "Saturated Sine", "Triangle", "Square", "Pulse", "Saw"]


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def load_params():
    params = []
    for line in (TABLES / "params.tsv").read_text().splitlines():
        f = line.split("\t")
        params.append(
            dict(
                index=int(f[0]),
                name=f[1],
                label=f[2],
                units=f[3].strip(),
                min=float(f[4]),
                max=float(f[5]),
                default=float(f[6]),
                scale=int(f[7]),
                offset=float(f[8]),
                mult=float(f[9]),
                invert=int(f[10]),
                items=f[12:],
            )
        )
    return params


def group_key(name: str):
    """The order of the host's params: the groups of the card's pages, each in Vital's own order."""
    m = re.match(r"(osc|env|lfo|random|filter)_(\d+)_", name)
    order = [
        "osc_1_", "osc_2_", "osc_3_", "sample_", "sub_", "filter_1_", "filter_2_",
        "env_", "lfo_", "random_", "macro_",
        "chorus_", "compressor_", "delay_", "distortion_", "eq_", "filter_fx_", "flanger_", "phaser_", "reverb_",
    ]  # fmt: skip
    for i, prefix in enumerate(order):
        if name.startswith(prefix):
            number = re.match(r"[a-z_]+?(\d+)", name)
            return (i, int(number.group(1)) if prefix in ("env_", "lfo_", "random_", "macro_") and number else 0)
    return (len(order), 0)  # voice and global


def labelled(p):
    return p["label"].strip()


def source_label(name: str) -> str:
    fixed = {
        "aftertouch": "Aftertouch",
        "lift": "Release Velocity",
        "mod_wheel": "Mod Wheel",
        "note": "Note",
        "note_in_octave": "Note in Octave",
        "pitch_wheel": "Pitch Wheel",
        "random": "Random",
        "slide": "Slide",
        "stereo": "Stereo",
        "velocity": "Velocity",
    }
    if name in fixed:
        return fixed[name]
    m = re.fullmatch(r"(env|lfo|random|macro_control)_(\d+)", name)
    kind = {"env": "Envelope", "lfo": "LFO", "random": "Random LFO", "macro_control": "Macro"}[m.group(1)]
    return f"{kind} {m.group(2)}"


def menu_items(p):
    items = p["items"]
    lo, hi = int(p["min"]), int(p["max"])
    if p["name"] == "sub_waveform":
        items = SUB_WAVEFORMS
    if not items:
        return None
    return items[lo : hi + 1]


def display(p):
    """The `Display` law of a continuous param, written as a Rust expression."""
    lo, hi, mult, scale = p["min"], p["max"], p["mult"], p["scale"]
    unit = " " + p["units"] if p["units"] else ""
    name = p["name"]

    def fmt(x):
        return repr(float(x))

    def decimals(span):
        span = abs(span)
        return 0 if span >= 1000 else 1 if span >= 100 else 2 if span >= 1 else 3

    if scale == 0:  # indexed: whole numbers
        count = int(hi - lo) + 1
        return f'Display::Steps {{ min: {int(lo * mult)}, count: {count}, fmt: Fmt::new(0, {rust_str(unit)}) }}'
    if name.endswith(("_cutoff", "_center")) and (lo, hi) == (8, 136):
        f = lambda n: 440.0 * 2 ** ((n - 69) / 12)
        return f"Display::Freq {{ min: {fmt(f(lo))}, max: {fmt(f(hi))}, hz_decimals: 0, khz_decimals: 2 }}"
    if scale == 1:
        a, b = lo * mult, hi * mult
        return f"Display::linear({fmt(a)}, {fmt(b)}, {decimals(b - a)}, {rust_str(unit)})"
    if scale in (2, 3, 4):
        # Vital shows value^k, times its multiplier; the travel is the value itself.
        a, b = lo, hi
        top = (b**scale) * mult
        bottom = (a**scale) * mult
        if a == 0:
            return f"Display::power(0.0, {fmt(top)}, {scale}, {decimals(top)}, {rust_str(unit)})"
        return f"Display::linear({fmt(bottom)}, {fmt(top)}, {decimals(top - bottom)}, {rust_str(unit)})"
    if scale == 6:
        # 2^value, times its multiplier: a rate in Hz or a time.
        a, b = 2**lo * mult, 2**hi * mult
        if "secs" in unit and mult == 1.0 and p["invert"]:
            # a period shown as the rate it is
            return f'Display::Freq {{ min: {fmt(a)}, max: {fmt(b)}, hz_decimals: 2, khz_decimals: 2 }}'
        shown = unit
        return f"Display::exp({fmt(a)}, {fmt(b)}, {decimals(b)}, {rust_str(shown)})"
    if scale == 5:
        return "Display::Custom(volume_text)"
    return f"Display::linear({fmt(lo)}, {fmt(hi)}, 2, {rust_str(unit)})"


def main():
    params = load_params()
    by_name = {p["name"]: p for p in params}
    sources = (TABLES / "mod_sources.txt").read_text().split()
    destinations = [
        d
        for d in (TABLES / "mod_destinations.txt").read_text().split()
        if not d.startswith("modulation_") and d != "beats_per_minute"
    ]

    shown = []
    for p in params:
        n = p["name"]
        if n in EXCLUDED or re.fullmatch(r"osc_\d+_view_2d", n) or n.startswith("modulation_"):
            continue
        shown.append(p)
    shown.sort(key=lambda p: (group_key(p["name"]), p["index"]))

    # Entries: ("param", vital index, name, label) …, plus the three wavetable menus after each oscillator's own
    # params and the matrix slots at the end.
    entries = []  # dicts: kind, host name, vital param or None, ...
    osc_end = {}
    for p in shown:
        entries.append(dict(kind="param", p=p))
    # Wavetable menus right after the last param of oscillator n.
    for n in range(1, WAVETABLES + 1):
        last = max(i for i, e in enumerate(entries) if e["kind"] == "param" and e["p"]["name"].startswith(f"osc_{n}_"))
        entries.insert(last + 1, dict(kind="wavetable", osc=n - 1, name=f"Oscillator {n} Wavetable"))
    for s in range(1, SLOTS + 1):
        entries.append(dict(kind="source", slot=s - 1, name=f"Modulation {s} Source"))
        entries.append(dict(kind="destination", slot=s - 1, name=f"Modulation {s} Destination"))
        for field in ("amount", "power", "bipolar", "stereo", "bypass"):
            entries.append(dict(kind="param", p=by_name[f"modulation_{s}_{field}"]))

    # ---- the engine side
    out = [
        "//! Generated by `tools/gen_vital_tables.py` from Vital's own parameter list (`vendor/vital/tables`): do not edit.",
        "//!",
        "//! The host's params of the Vital module (after Enable, p0), each as the Vital param it sets and that param's range.",
        "#![allow(clippy::approx_constant)] // Vital's own ranges, written as the engine reports them",
        "",
        "use super::vital::{Entry, Role};",
        "use std::ffi::CStr;",
        "",
        f"pub const SLOTS: usize = {SLOTS};",
        f"/// How many Vital parameters there are in all (the matrix's other slots are not shown).",
        f"pub const VITAL_PARAMS: usize = {len(params)};",
        "",
        f"pub static TABLE: [Entry; {len(entries)}] = [",
    ]
    for e in entries:
        if e["kind"] == "param":
            p = e["p"]
            items = menu_items(p)
            out.append(
                f"    Entry {{ name: {rust_str(p['name'])}, role: Role::Param({p['index']}), "
                f"min: {float(p['min'])!r}, max: {float(p['max'])!r}, menu: {len(items) if items else 0}, indexed: {'true' if p['scale'] == 0 else 'false'} }},"
            )
        elif e["kind"] == "wavetable":
            out.append(
                f"    Entry {{ name: \"osc_{e['osc'] + 1}_wavetable\", role: Role::Wavetable({e['osc']}), min: 0.0, max: 0.0, menu: 0, indexed: false }},"
            )
        elif e["kind"] == "source":
            out.append(
                f"    Entry {{ name: \"modulation_{e['slot'] + 1}_source\", role: Role::Source({e['slot']}), min: 0.0, max: {float(len(sources))!r}, menu: {len(sources) + 1}, indexed: true }},"
            )
        else:
            out.append(
                f"    Entry {{ name: \"modulation_{e['slot'] + 1}_destination\", role: Role::Destination({e['slot']}), min: 0.0, max: {float(len(destinations))!r}, menu: {len(destinations) + 1}, indexed: true }},"
            )
    out.append("];")
    out.append("")
    out.append(f"pub static SOURCES: [&CStr; {len(sources)}] = [")
    out += [f"    c{rust_str(s)}," for s in sources]
    out.append("];")
    out.append("")
    out.append(f"pub static DESTINATIONS: [&CStr; {len(destinations)}] = [")
    out += [f"    c{rust_str(d)}," for d in destinations]
    out.append("];")
    (ROOT / "crates/audio/src/vital_table.rs").write_text("\n".join(out) + "\n")

    # ---- the registry side
    reg = [
        "//! Generated by `tools/gen_vital_tables.py` from Vital's own parameter list (`vendor/vital/tables`): do not edit.",
        "#![allow(clippy::approx_constant)] // Vital's own ranges, written as the engine reports them",
        "",
        "use super::display::{Display, Fmt, menu_value};",
        "use honeydew_audio::vital::WAVETABLES;",
        "use super::ParamSpec as P;",
        "",
        "/// `Volume`: Vital's value is the square root of its power above −80 dB.",
        "fn volume_text(v: f32) -> String {",
        "    format!(\"{:.1} dB\", (f64::from(v) * 7399.44).sqrt() - 80.0)",
        "}",
        "",
    ]
    names_src = [source_label(s) for s in sources]
    names_dst = [labelled(by_name[d]) for d in destinations]
    reg.append(f"const SOURCE_ITEMS: [&str; {len(sources) + 1}] = [\"None\", " + ", ".join(rust_str(s) for s in names_src) + "];")
    reg.append(
        f"const DESTINATION_ITEMS: [&str; {len(destinations) + 1}] = [\"None\", "
        + ", ".join(rust_str(s) for s in names_dst)
        + "];"
    )
    reg.append("")
    reg.append(f"/// Enable, then {len(entries)} params in the order of `honeydew_audio::vital::TABLE`.")
    reg.append(f"pub static PARAMS: [P; {len(entries) + 1}] = [")
    reg.append("    P::ENABLE,")
    for e in entries:
        if e["kind"] == "param":
            p = e["p"]
            items = menu_items(p)
            label = labelled(p)
            default = p["default"]
            span = p["max"] - p["min"]
            if items:
                index = int(round(default - p["min"]))
                if items == ["Off", "On"]:
                    reg.append(f"    P::toggle({rust_str(label)}, {float(index)!r}),")
                else:
                    ident = "&[" + ", ".join(rust_str(i) for i in items) + "]"
                    reg.append(f"    P::menu({rust_str(label)}, menu_value({index}, {len(items)}), {ident}),")
            else:
                norm = (default - p["min"]) / span if span else 0.0
                reg.append(f"    P::cont({rust_str(label)}, {norm!r}, {display(p)}),")
        elif e["kind"] == "wavetable":
            reg.append(f"    P::menu({rust_str(e['name'])}, 0.0, &WAVETABLES),")
        elif e["kind"] == "source":
            reg.append(f"    P::menu({rust_str(e['name'])}, 0.0, &SOURCE_ITEMS),")
        else:
            reg.append(f"    P::menu({rust_str(e['name'])}, 0.0, &DESTINATION_ITEMS),")
    reg.append("];")
    reg.append("")
    (ROOT / "src/params/vital_table.rs").write_text("\n".join(reg) + "\n")
    print(f"{len(entries)} host params ({len(shown)} Vital params shown of {len(params)}), {len(sources)} sources, {len(destinations)} destinations")


if __name__ == "__main__":
    sys.exit(main())
