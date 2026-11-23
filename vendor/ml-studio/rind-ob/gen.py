#!/usr/bin/env python3
"""Writes src/settings.rs and banks/presets.bin for Rind OB from OB-Xf's factory patches.

usage: gen.py <OB-Xf>/assets/installer/"Surge Synth Team"/OB-Xf/Patches

A setting is an OB-Xf parameter, stored as its normalised value x 1000 (what OB-Xf's own patches hold, so a patch loads
1:1); the discrete parameters are snapped to the value the engine reads them as."""
import glob, os, re, struct, sys

# (xml id, name, kind, default normalised) kinds: f float, b bool, t tri-state, v voices, l legato, p priority,
# x xpander mode, n noise colour
P = [
    ("Volume", "Volume", "f", .5), ("Transpose", "Transpose", "f", .5), ("Tune", "Tune", "f", .5),
    ("Polyphony", "Polyphony", "v", 7.5 / 32), ("HQMode", "High Quality", "b", 0), ("UnisonVoices", "Unison Voices", "v", 7.5 / 32),
    ("Portamento", "Glide", "f", 0), ("Unison", "Unison", "b", 0), ("UnisonDetune", "Unison Detune", "f", .25),
    ("EnvLegatoMode", "Envelope Legato", "l", 0), ("NotePriority", "Note Priority", "p", 0), ("VoiceReassign", "Voice Reassign", "b", 0),
    ("Osc1Pitch", "Osc 1 Pitch", "f", .5), ("Osc2Detune", "Osc 2 Detune", "f", 0), ("Osc2Pitch", "Osc 2 Pitch", "f", .5),
    ("Osc2Keytrack", "Osc 2 Keytrack", "b", 1),
    ("Osc1SawWave", "Osc 1 Saw", "b", 1), ("Osc1PulseWave", "Osc 1 Pulse", "b", 0), ("Osc2SawWave", "Osc 2 Saw", "b", 1),
    ("Osc2PulseWave", "Osc 2 Pulse", "b", 0), ("OscPW", "Pulse Width", "f", 0), ("Osc2PWOffset", "Osc 2 PW Offset", "f", 0),
    ("EnvToPitchAmount", "Env to Pitch", "f", 0), ("EnvToPitchBothOscs", "Env Pitch Both Oscs", "b", 1),
    ("EnvToPitchInvert", "Env Pitch Invert", "b", 0), ("EnvToPWAmount", "Env to PW", "f", 0),
    ("EnvToPWBothOscs", "Env PW Both Oscs", "b", 1), ("EnvToPWInvert", "Env PW Invert", "b", 0),
    ("OscCrossmod", "Crossmod", "f", 0), ("OscSync", "Osc Sync", "b", 0), ("OscBrightness", "Brightness", "f", 1),
    ("Osc1Mix", "Osc 1 Mix", "f", 1), ("Osc2Mix", "Osc 2 Mix", "f", 0), ("RingModMix", "Ring Mod Mix", "f", 0),
    ("NoiseMix", "Noise Mix", "f", 0), ("NoiseColor", "Noise Colour", "n", 0),
    ("PitchBendUp", "Bend Up", "f", 2 / 48), ("PitchBendDown", "Bend Down", "f", 2 / 48), ("BendOsc2Only", "Bend Osc 2 Only", "b", 0),
    ("VibratoWave", "Vibrato Wave", "b", 0), ("VibratoRate", "Vibrato Rate", "f", .3),
    ("Filter4PoleMode", "Filter 4-Pole", "b", 0), ("FilterCutoff", "Cutoff", "f", 1), ("FilterResonance", "Resonance", "f", 0),
    ("FilterEnvAmount", "Filter Env Amount", "f", 0), ("FilterKeyFollow", "Filter Keytrack", "f", 0),
    ("FilterMode", "Filter Mode", "f", 0), ("Filter2PoleBPBlend", "2-Pole BP Blend", "b", 0),
    ("Filter2PolePush", "2-Pole Push", "b", 0), ("Filter4PoleXpander", "Xpander", "b", 0), ("FilterXpanderMode", "Xpander Mode", "x", 0),
]
for n in (1, 2):
    P += [
        (f"LFO{n}TempoSync", f"LFO {n} Sync", "b", 0), (f"LFO{n}Rate", f"LFO {n} Rate", "f", .5),
        (f"LFO{n}ModAmount1", f"LFO {n} Amount 1", "f", 0), (f"LFO{n}ModAmount2", f"LFO {n} Amount 2", "f", 0),
        (f"LFO{n}Wave1", f"LFO {n} Sine/Tri", "f", 0), (f"LFO{n}Wave2", f"LFO {n} Pulse/Saw", "f", .5),
        (f"LFO{n}Wave3", f"LFO {n} S&H/Glide", "f", .5), (f"LFO{n}PW", f"LFO {n} Pulse Width", "f", 0),
        (f"LFO{n}ToOsc1Pitch", f"LFO {n} to Osc 1 Pitch", "t", 0), (f"LFO{n}ToOsc2Pitch", f"LFO {n} to Osc 2 Pitch", "t", 0),
        (f"LFO{n}ToFilterCutoff", f"LFO {n} to Cutoff", "t", 0), (f"LFO{n}ToOsc1PW", f"LFO {n} to Osc 1 PW", "t", 0),
        (f"LFO{n}ToOsc2PW", f"LFO {n} to Osc 2 PW", "t", 0), (f"LFO{n}ToVolume", f"LFO {n} to Volume", "t", 0),
    ]
P += [
    ("FilterEnvInvert", "Filter Env Invert", "b", 0), ("FilterEnvAttack", "Filter Attack", "f", 0), ("FilterEnvDecay", "Filter Decay", "f", 0),
    ("FilterEnvSustain", "Filter Sustain", "f", 1), ("FilterEnvRelease", "Filter Release", "f", 0),
    ("FilterEnvAttackCurve", "Filter Attack Curve", "f", 0), ("VelToFilterEnv", "Velocity to Filter", "f", 0),
    ("AmpEnvAttack", "Amp Attack", "f", 0), ("AmpEnvDecay", "Amp Decay", "f", 0), ("AmpEnvSustain", "Amp Sustain", "f", 1),
    ("AmpEnvRelease", "Amp Release", "f", 0), ("AmpEnvAttackCurve", "Amp Attack Curve", "f", 0),
    ("VelToAmpEnv", "Velocity to Amp", "f", 0),
    ("PortamentoSlop", "Glide Slop", "f", .25), ("FilterSlop", "Filter Slop", "f", .25), ("EnvelopeSlop", "Envelope Slop", "f", .25),
    ("LevelSlop", "Level Slop", "f", .25),
]
P += [(f"PanVoice{n}", f"Pan Voice {n}", "f", .5) for n in range(1, 9)]


def snake(s):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", s).lower().replace("l_f_o", "lfo")


def snap(kind, v):
    """The stored value (0..1000) of a patch's normalised value, as the engine reads it."""
    v = min(1.0, max(0.0, v))
    if kind == "b":
        return 1000 if v >= 0.5 else 0
    if kind == "t":
        return round(v * 2) * 500
    if kind == "v":
        n = 1 + int(v * 32)
        return round((n - 0.5) / 32 * 1000)
    if kind == "l":
        return [100, 400, 700, 1000][min(3, int(v * 3))]
    if kind == "p":
        return [100, 500, 1000][int(v * 2)]
    if kind == "x":
        return round(round(v * 14) / 14 * 1000)
    if kind == "n":
        return 100 if v < 1 / 3 else 500 if v < 2 / 3 else 900
    return round(v * 1000)


def main(src):
    root = os.path.dirname(os.path.abspath(__file__))
    ids = {p[0] for p in P}
    cats, authors, presets = [], [], []
    for f in sorted(glob.glob(os.path.join(src, "*", "*.fxp"))):
        b = open(f, "rb").read()
        x = b[b.find(b"<?xml"):].decode("utf8", "replace")
        head = x[: x.find("/>")]
        a = dict(re.findall(r'([\w-]+)="([^"]*)"', head))
        cat = a.get("category") or os.path.basename(os.path.dirname(f))
        au = a.get("author") or "OB-Xf"
        name = a.get("programName") or os.path.splitext(os.path.basename(f))[0]
        if cat not in cats:
            cats.append(cat)
        if au not in authors:
            authors.append(au)
        vals = []
        for (i, _, kind, d) in P:
            vals.append(snap(kind, float(a[i])) if i in a else snap(kind, d))
        presets.append((cats.index(cat), authors.index(au), name[:60], vals))
    presets.sort(key=lambda p: (p[0], p[2].lower()))
    out = bytearray(struct.pack("<HHHH", len(P), len(cats), len(authors), len(presets)))
    for s in cats + authors:
        e = s.encode()
        out += bytes([len(e)]) + e
    for (c, au, name, vals) in presets:
        e = name.encode()
        out += bytes([c, au, len(e)]) + e + struct.pack(f"<{len(vals)}H", *vals)
    open(os.path.join(root, "banks", "presets.bin"), "wb").write(out)
    rows = [
        "// Generated by gen.py: Rind OB's settings, one per OB-Xf parameter, stored as the normalised value x 1000.",
        "use ml_small_fx::Setting;", "", f"pub const COUNT: usize = {len(P)};", "",
        f"pub static SETTINGS: [Setting; {len(P)}] = [",
    ]
    for (i, name, kind, d) in P:
        st = snap(kind, d)
        mg = "Some(500)" if i.startswith("Pan") or i in ("Transpose", "Tune", "Osc1Pitch", "Osc2Pitch") else "None"
        rows.append(f'    Setting {{ id: "{snake(i)}", name: "{name}", range: (0, 1000), start: {st}, magnet: {mg} }},')
    rows += ["];", "", f"pub static IDS: [&str; {len(P)}] = ["]
    rows += [f'    "{snake(i)}",' for (i, *_) in P]
    rows += ["];", "", "/// OB-Xf's name for each setting, NUL-ended (`obx_param` takes it), and what kind of value it is.",
             f"pub static XML: [&[u8]; {len(P)}] = ["]
    rows += [f'    b"{i}\\0",' for (i, *_) in P]
    rows += ["];", "", f"pub static KINDS: [u8; {len(P)}] = ["]
    rows += ["    " + ", ".join(f"b'{k}'" for (_, _, k, _) in P[j:j + 12]) + "," for j in range(0, len(P), 12)]
    rows += ["];", ""]
    rows += [f"pub const {snake(i).upper()}: usize = {n};" for n, (i, *_) in enumerate(P)]
    open(os.path.join(root, "src", "settings.rs"), "w").write("\n".join(rows) + "\n")
    print(len(P), "settings", len(presets), "presets", len(out), "bytes", cats)


main(sys.argv[1])
