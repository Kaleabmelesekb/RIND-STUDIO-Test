#!/usr/bin/env python3
"""Writes crates/rind/src/preset_table.rs: Rind's factory patches as recipes (see presets.rs for what each field does).

Every category has six archetypes, each in five variations built from the category's own adjectives, so no two patches
set the same values. Run from the repository root:  python3 tools/gen_rind_presets.py
"""
import re

# ---- the fields of a recipe, with the value `presets::BASE` gives them ------------------------------------------
BASE = dict(
    w1=1, v1=1, d1=0.2, o1=0, w2=1, v2=0, d2=0.2, o2=0, s2=0, l2=0.75, st=0.5,
    a=0.003, d=0.6, s=1.0, r=0.25,
    ft=0, fs=1, cut=12000.0, res=0.1, drv=0.0, key=0.0,
    fe=0.0, ea=0.003, ed=0.4, es=0.5, er=0.3,
    lw=0, lr=2.0, lt=0, la=0.0, vib=0.0,
    mono=0, porta=0.0,
    dty=0, dist=0.0, cho=0.0, phs=0.0, dly=0.0, dfb=0.35, rev=0.0, rsz=0.6,
    bass=0.0, treb=0.0, comp=0,
    arp=0, div=11, aoct=1, gate=0.5, pat=0,
)
INT = {k for k in BASE if k in (
    'w1', 'v1', 'o1', 'w2', 'v2', 'o2', 's2', 'ft', 'fs', 'lw', 'lt', 'mono', 'dty', 'comp', 'arp', 'div', 'aoct', 'pat')}
RANGE = dict(
    d1=(0, 1), d2=(0, 1), l2=(0, 1), st=(0, 1), a=(0.001, 10), d=(0.001, 10), s=(0, 1), r=(0.001, 10),
    cut=(20, 20000), res=(0, 1), drv=(0, 1), key=(0, 1), fe=(-1, 1), ea=(0.001, 10), ed=(0.001, 10), es=(0, 1),
    er=(0.001, 10), lr=(0.02, 40), la=(-1, 1), vib=(0, 0.2), porta=(0, 2), dist=(0, 1), cho=(0, 1), phs=(0, 1),
    dly=(0, 1), dfb=(0, 0.95), rev=(0, 1), rsz=(0, 1), bass=(-12, 12), treb=(-12, 12), gate=(0.05, 1),
)
INTRANGE = dict(w1=(0, 7), v1=(0, 8), o1=(-3, 3), w2=(0, 7), v2=(0, 8), o2=(-3, 3), s2=(-12, 12), ft=(0, 2),
                fs=(0, 1), lw=(0, 10), lt=(0, 34), mono=(0, 1), dty=(0, 4), comp=(0, 1), arp=(0, 10), div=(0, 14),
                aoct=(1, 4), pat=(0, 5))

# wave names
SINE, SAW, TRI, PULSE, HALF, QUART, TRISAW, NOISE = range(8)
# lfo targets (mtgt)
T_CUT, T_RES, T_PITCH, T_DETUNE, T_AMP, T_PAN, T_LVL2 = 1, 3, 7, 14, 19, 32, 31


def apply(rec, mods):
    out = dict(rec)
    for k, v in mods.items():
        if isinstance(v, tuple):
            op, x = v
            out[k] = out[k] * x if op == '*' else out[k] + x
        else:
            out[k] = v
    return out


def clean(rec):
    out = {}
    for k, v in rec.items():
        if k in INT:
            lo, hi = INTRANGE[k]
            v = int(max(lo, min(hi, round(v))))
        else:
            lo, hi = RANGE[k]
            v = round(max(lo, min(hi, v)), 4)
        if v != BASE[k]:
            out[k] = v
    return out


def mul(x): return ('*', x)
def add(x): return ('+', x)


# ---- the categories: (name, archetypes, adjectives) ------------------------------------------------------------
# An archetype is (name, base mods); an adjective is (word, mods) applied on top.
CATS = []

CATS.append(('Bass', [
    ('Sub', dict(w1=SINE, o1=-1, v1=1, w2=SINE, v2=1, o2=-2, l2=0.5, mono=1, cut=600, a=0.003, d=0.4, s=0.9, r=0.12, porta=0.04)),
    ('Saw Bass', dict(w1=SAW, o1=-1, v1=2, d1=0.15, mono=1, cut=700, res=0.25, fe=0.35, ed=0.25, es=0.2, a=0.002, d=0.4, s=0.8, r=0.15)),
    ('Reese', dict(w1=SAW, o1=-1, v1=3, d1=0.35, w2=SAW, v2=3, o2=-1, d2=0.4, s2=0, mono=1, cut=900, res=0.2, st=0.8, a=0.01, d=0.5, s=0.9, r=0.3, cho=0.3)),
    ('Acid', dict(w1=SAW, o1=-1, mono=1, porta=0.08, cut=500, res=0.7, fe=0.55, ed=0.18, es=0.1, a=0.002, d=0.25, s=0.5, r=0.1, drv=0.3, dist=0.2)),
    ('Growl', dict(w1=PULSE, o1=-1, v1=2, d1=0.3, w2=SAW, v2=1, o2=0, l2=0.5, mono=1, cut=1200, res=0.45, lt=T_CUT, lw=0, lr=3.0, la=0.35, dist=0.45, dty=1, a=0.002, d=0.4, s=0.8, r=0.1)),
    ('Pluck Bass', dict(w1=SAW, o1=-1, v1=1, mono=1, cut=400, res=0.3, fe=0.5, ed=0.12, es=0.0, a=0.001, d=0.18, s=0.0, r=0.1)),
], [
    ('Deep', dict(cut=mul(0.7), r=mul(1.3))),
    ('Tight', dict(d=mul(0.5), r=mul(0.5), cut=mul(1.2))),
    ('Fat', dict(d1=add(0.15), v1=add(1), bass=4)),
    ('Dirty', dict(dist=add(0.35), drv=add(0.2), res=add(0.15))),
    ('Bright', dict(cut=mul(2.2), fe=add(0.15), treb=3)),
    ('Wide', dict(st=1.0, cho=0.35, v1=add(1))),
    ('Boom', dict(bass=6, s=1.0, d=mul(1.6), r=mul(1.5))),
    ('Glide', dict(porta=0.12, mono=1)),
    ('Punch', dict(comp=1, fe=add(0.2), ed=mul(0.6), a=0.001)),
    ('Vintage', dict(cut=mul(0.8), d1=add(0.1), phs=0.15, treb=-2)),
]))

CATS.append(('Lead', [
    ('Saw Lead', dict(w1=SAW, v1=3, d1=0.25, mono=1, porta=0.03, cut=5000, res=0.2, a=0.003, d=0.4, s=0.85, r=0.2, vib=0.01, dly=0.2)),
    ('Square Lead', dict(w1=PULSE, v1=2, d1=0.2, mono=1, porta=0.03, cut=4500, res=0.25, a=0.003, d=0.4, s=0.8, r=0.18, vib=0.01, dly=0.2)),
    ('Super Lead', dict(w1=SAW, v1=6, d1=0.4, w2=SAW, v2=4, o2=1, d2=0.3, l2=0.4, cut=6500, res=0.15, st=0.9, a=0.004, d=0.5, s=0.85, r=0.3, cho=0.3, dly=0.25, rev=0.15)),
    ('Sync Lead', dict(w1=SAW, v1=2, d1=0.1, w2=SAW, v2=1, s2=7, l2=0.5, mono=1, cut=7000, res=0.3, lt=T_PITCH, lw=1, lr=0.6, la=0.02, a=0.003, d=0.3, s=0.8, r=0.15)),
    ('Pluck Lead', dict(w1=SAW, v1=3, d1=0.15, cut=3000, res=0.4, fe=0.4, ed=0.2, es=0.1, a=0.002, d=0.3, s=0.0, r=0.25, dly=0.3, rev=0.2)),
    ('Soft Lead', dict(w1=TRI, v1=2, d1=0.1, w2=SINE, v2=1, o2=1, l2=0.3, mono=1, porta=0.05, cut=3500, a=0.03, d=0.4, s=0.9, r=0.35, vib=0.012, rev=0.25)),
], [
    ('Bright', dict(cut=mul(1.6), treb=3)),
    ('Dark', dict(cut=mul(0.5), res=add(0.1))),
    ('Wide', dict(st=1.0, v1=add(1), cho=0.4)),
    ('Screamer', dict(dist=0.5, dty=2, res=add(0.2), drv=0.4)),
    ('Vintage', dict(phs=0.2, d1=add(0.1), cut=mul(0.8))),
    ('Long', dict(r=mul(2.5), rev=add(0.2), d=mul(1.5))),
    ('Short', dict(r=mul(0.4), d=mul(0.6), s=mul(0.6))),
    ('Echo', dict(dly=0.45, dfb=0.5)),
    ('Glide', dict(mono=1, porta=0.15)),
    ('Vibrato', dict(vib=0.03, a=add(0.02))),
]))

CATS.append(('Pad', [
    ('Warm Pad', dict(w1=SAW, v1=4, d1=0.3, w2=TRI, v2=2, o2=0, l2=0.5, cut=2500, a=0.8, d=0.6, s=0.85, r=1.4, cho=0.4, rev=0.3, st=0.9)),
    ('Glass Pad', dict(w1=TRI, v1=3, d1=0.2, w2=SINE, v2=2, o2=1, l2=0.5, cut=6000, a=0.6, d=0.7, s=0.8, r=1.5, phs=0.3, rev=0.4, st=0.9)),
    ('Analog Pad', dict(w1=PULSE, v1=3, d1=0.35, w2=SAW, v2=2, o2=-1, l2=0.5, cut=1800, res=0.15, a=0.5, d=0.8, s=0.8, r=1.2, lt=T_CUT, lw=0, lr=0.25, la=0.15, cho=0.35, rev=0.25)),
    ('Wide Pad', dict(w1=SAW, v1=7, d1=0.5, w2=SAW, v2=6, o2=1, d2=0.45, l2=0.4, cut=3000, a=1.0, d=0.8, s=0.85, r=1.8, cho=0.5, rev=0.4, st=1.0)),
    ('Sweep Pad', dict(w1=SAW, v1=4, d1=0.3, cut=500, res=0.3, fe=0.45, ea=1.5, ed=1.5, es=0.5, er=1.5, a=1.2, d=1.0, s=0.8, r=1.5, rev=0.35, cho=0.3, st=0.9)),
    ('Soft Pad', dict(w1=TRI, v1=2, d1=0.2, w2=SINE, v2=1, l2=0.6, cut=2000, a=1.2, d=0.6, s=0.9, r=2.0, rev=0.45, cho=0.2, st=0.7)),
], [
    ('Slow', dict(a=mul(1.8), r=mul(1.5))),
    ('Dark', dict(cut=mul(0.5), treb=-3)),
    ('Bright', dict(cut=mul(1.9), treb=3)),
    ('Lush', dict(cho=0.55, rev=0.5, v1=add(1))),
    ('Moving', dict(lt=T_CUT, lw=1, lr=0.18, la=0.25)),
    ('Phased', dict(phs=0.45)),
    ('Airy', dict(treb=4, cut=mul(1.5), rev=0.55, rsz=0.85)),
    ('Vintage', dict(d1=add(0.1), cut=mul(0.7), phs=0.2, bass=2)),
    ('Cinematic', dict(rev=0.6, rsz=0.95, a=mul(2.2), r=mul(2.0), bass=3)),
    ('Evolving', dict(lt=T_DETUNE, lw=8, lr=0.3, la=0.4, fe=add(0.15))),
]))

CATS.append(('Pluck', [
    ('Saw Pluck', dict(w1=SAW, v1=2, d1=0.2, cut=2500, res=0.25, fe=0.5, ed=0.18, es=0.0, a=0.001, d=0.28, s=0.0, r=0.25)),
    ('Square Pluck', dict(w1=PULSE, v1=1, cut=2200, res=0.2, fe=0.45, ed=0.15, es=0.0, a=0.001, d=0.25, s=0.0, r=0.2)),
    ('Pizz', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=1, l2=0.4, cut=3500, a=0.001, d=0.18, s=0.0, r=0.2, rev=0.2)),
    ('Harp', dict(w1=TRI, v1=2, d1=0.1, w2=SINE, v2=1, o2=2, l2=0.3, cut=5000, a=0.001, d=0.9, s=0.0, r=0.8, rev=0.35, dly=0.2)),
    ('Marimba', dict(w1=SINE, v1=1, w2=SINE, v2=1, o2=2, s2=4, l2=0.25, cut=6000, a=0.001, d=0.4, s=0.0, r=0.3, rev=0.2)),
    ('Kalimba', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=2, l2=0.35, cut=4500, a=0.001, d=0.55, s=0.0, r=0.5, dly=0.3, rev=0.3)),
], [
    ('Short', dict(d=mul(0.5), r=mul(0.5))),
    ('Long', dict(d=mul(2.2), r=mul(2.0), rev=add(0.2))),
    ('Bright', dict(cut=mul(1.8), treb=3)),
    ('Soft', dict(cut=mul(0.55), fe=mul(0.5))),
    ('Wide', dict(st=1.0, cho=0.35, v1=add(1))),
    ('Echo', dict(dly=0.4, dfb=0.5)),
    ('Dry', dict(rev=0.0, dly=0.0, r=mul(0.6))),
    ('Detuned', dict(d1=add(0.25), v1=add(1))),
    ('Octave', dict(o2=1, v2=1, l2=0.45)),
    ('Phased', dict(phs=0.35)),
]))

CATS.append(('Keys', [
    ('Electric Piano', dict(w1=SINE, v1=1, w2=TRI, v2=1, o2=2, l2=0.25, cut=5000, fe=0.15, ed=0.4, es=0.2, a=0.002, d=1.2, s=0.25, r=0.5, cho=0.3, phs=0.15)),
    ('Organ', dict(w1=SINE, v1=1, w2=SINE, v2=1, o2=1, l2=0.6, cut=7000, a=0.005, d=0.1, s=1.0, r=0.08, lt=T_AMP, lw=0, lr=6.0, la=0.04, cho=0.3, rev=0.2)),
    ('Clav', dict(w1=PULSE, v1=1, cut=2400, res=0.3, fe=0.4, ed=0.1, es=0.1, a=0.001, d=0.2, s=0.15, r=0.1, treb=2)),
    ('Wurly', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=1, l2=0.3, cut=3200, res=0.1, a=0.002, d=0.9, s=0.3, r=0.4, dist=0.2, dty=0, cho=0.25)),
    ('Synth Keys', dict(w1=SAW, v1=2, d1=0.2, w2=PULSE, v2=1, l2=0.4, cut=3500, res=0.2, fe=0.25, ed=0.35, es=0.4, a=0.003, d=0.6, s=0.6, r=0.3, cho=0.25, rev=0.2)),
    ('Soft Piano', dict(w1=TRI, v1=2, d1=0.08, w2=SINE, v2=1, o2=1, l2=0.3, cut=3000, fe=0.2, ed=0.5, es=0.1, a=0.002, d=1.6, s=0.1, r=0.7, rev=0.3)),
], [
    ('Bright', dict(cut=mul(1.7), treb=3)),
    ('Mellow', dict(cut=mul(0.55), treb=-3)),
    ('Wide', dict(st=1.0, cho=add(0.2), v1=add(1))),
    ('Tremolo', dict(lt=T_AMP, lw=0, lr=4.5, la=0.25)),
    ('Vintage', dict(d1=add(0.1), phs=0.25, dist=add(0.1))),
    ('Long', dict(d=mul(1.8), r=mul(1.8), rev=add(0.2))),
    ('Dry', dict(rev=0.0, dly=0.0, r=mul(0.6))),
    ('Phased', dict(phs=0.4)),
    ('Dirty', dict(dist=add(0.3), drv=add(0.15))),
    ('Echo', dict(dly=0.35, dfb=0.45)),
]))

CATS.append(('Arp', [
    ('Classic Arp', dict(w1=SAW, v1=2, d1=0.2, cut=3800, res=0.25, a=0.002, d=0.3, s=0.5, r=0.15, arp=1, div=11, aoct=2, gate=0.55, dly=0.25, rev=0.15)),
    ('Pulse Seq', dict(w1=PULSE, v1=2, d1=0.15, cut=2800, res=0.3, fe=0.3, ed=0.15, es=0.1, a=0.001, d=0.2, s=0.3, r=0.12, arp=1, div=11, aoct=2, gate=0.4, pat=2, dly=0.25)),
    ('Trance Gate', dict(w1=SAW, v1=5, d1=0.35, w2=SAW, v2=3, o2=1, l2=0.4, cut=5000, res=0.15, a=0.002, d=0.3, s=0.7, r=0.15, arp=1, div=11, aoct=1, gate=0.35, pat=3, dly=0.3, rev=0.2, st=0.9)),
    ('Acid 303', dict(w1=SAW, v1=1, mono=1, porta=0.03, cut=700, res=0.6, fe=0.5, ed=0.2, es=0.1, a=0.001, d=0.25, s=0.4, r=0.1, drv=0.3, dist=0.15, arp=1, div=11, aoct=1, gate=0.6, pat=1, dly=0.3)),
    ('Bell Arp', dict(w1=SINE, v1=1, w2=TRI, v2=1, o2=2, l2=0.3, cut=7000, a=0.001, d=0.5, s=0.0, r=0.5, arp=1, div=8, aoct=3, gate=0.7, rev=0.4, dly=0.3)),
    ('Rising Seq', dict(w1=TRISAW, v1=3, d1=0.2, cut=3000, res=0.3, fe=0.3, ed=0.2, es=0.2, a=0.002, d=0.3, s=0.5, r=0.2, arp=1, div=11, aoct=2, gate=0.5, pat=4, dly=0.25, rev=0.2)),
], [
    ('Up', dict(arp=1)),
    ('Down', dict(arp=2)),
    ('Up Down', dict(arp=3)),
    ('Random', dict(arp=7)),
    ('Eighths', dict(div=8)),
    ('Triplet', dict(div=12, gate=0.45)),
    ('Thirty-two', dict(div=13, gate=0.35)),
    ('Wide Range', dict(aoct=4)),
    ('Syncopated', dict(pat=5, div=11)),
    ('Dotted', dict(div=10, dly=0.4)),
]))

CATS.append(('Strings', [
    ('Ensemble', dict(w1=SAW, v1=5, d1=0.3, w2=SAW, v2=4, o2=0, d2=0.25, l2=0.5, cut=3200, a=0.35, d=0.6, s=0.9, r=0.7, cho=0.5, rev=0.3, st=1.0, vib=0.006)),
    ('Violins', dict(w1=SAW, v1=4, d1=0.2, w2=SAW, v2=2, o2=1, l2=0.3, cut=5200, res=0.1, a=0.25, d=0.5, s=0.9, r=0.5, cho=0.35, rev=0.3, st=0.9, vib=0.014)),
    ('Cellos', dict(w1=SAW, v1=3, d1=0.2, o1=-1, w2=TRI, v2=1, o2=-1, l2=0.4, cut=1800, a=0.3, d=0.6, s=0.9, r=0.6, cho=0.3, rev=0.25, vib=0.012)),
    ('Pizzicato', dict(w1=SAW, v1=3, d1=0.15, cut=2600, res=0.15, fe=0.3, ed=0.12, es=0.0, a=0.001, d=0.2, s=0.0, r=0.15, rev=0.2, cho=0.2)),
    ('Tremolo Strings', dict(w1=SAW, v1=4, d1=0.25, cut=3600, a=0.15, d=0.4, s=0.9, r=0.4, lt=T_AMP, lw=0, lr=8.0, la=0.35, rev=0.3, cho=0.3, st=0.9)),
    ('Synth Strings', dict(w1=SAW, v1=6, d1=0.45, w2=PULSE, v2=3, l2=0.4, cut=2800, a=0.45, d=0.6, s=0.9, r=0.9, phs=0.3, cho=0.45, rev=0.3, st=1.0)),
], [
    ('Slow', dict(a=mul(2.0), r=mul(1.6))),
    ('Close', dict(a=mul(0.4), r=mul(0.6), rev=mul(0.5))),
    ('Bright', dict(cut=mul(1.7), treb=3)),
    ('Dark', dict(cut=mul(0.55), treb=-3)),
    ('Wide', dict(st=1.0, cho=add(0.2), v1=add(1))),
    ('Hall', dict(rev=0.6, rsz=0.9)),
    ('Vintage', dict(d1=add(0.12), phs=0.3, cut=mul(0.75))),
    ('Soaring', dict(vib=0.02, a=mul(1.4), rev=add(0.2))),
    ('Staccato', dict(d=mul(0.4), s=mul(0.3), r=mul(0.4))),
    ('Cinematic', dict(rev=0.65, rsz=0.95, bass=3, a=mul(1.6))),
]))

CATS.append(('Brass', [
    ('Brass Section', dict(w1=SAW, v1=4, d1=0.15, w2=SAW, v2=3, o2=0, l2=0.5, cut=1200, res=0.15, fe=0.55, ea=0.08, ed=0.25, es=0.7, a=0.06, d=0.3, s=0.85, r=0.2, cho=0.15, rev=0.2)),
    ('Trumpet', dict(w1=SAW, v1=2, d1=0.08, cut=1500, res=0.2, fe=0.5, ea=0.05, ed=0.2, es=0.7, a=0.04, d=0.25, s=0.85, r=0.15, mono=1, vib=0.01, rev=0.2)),
    ('Horns', dict(w1=SAW, v1=3, d1=0.12, o1=-1, w2=TRI, v2=1, o2=-1, l2=0.4, cut=900, res=0.1, fe=0.5, ea=0.1, ed=0.3, es=0.7, a=0.09, d=0.3, s=0.85, r=0.3, rev=0.25)),
    ('Synth Brass', dict(w1=SAW, v1=5, d1=0.3, w2=PULSE, v2=2, l2=0.4, cut=1600, res=0.2, fe=0.6, ea=0.05, ed=0.3, es=0.6, a=0.03, d=0.3, s=0.8, r=0.2, cho=0.3, st=0.9, rev=0.15)),
    ('Stabs', dict(w1=SAW, v1=4, d1=0.25, w2=SAW, v2=2, o2=1, l2=0.4, cut=2200, res=0.2, fe=0.4, ed=0.15, es=0.0, a=0.002, d=0.22, s=0.0, r=0.12, cho=0.2, dly=0.2)),
    ('Tuba', dict(w1=SAW, v1=2, d1=0.08, o1=-2, w2=SINE, v2=1, o2=-2, l2=0.5, cut=500, fe=0.4, ea=0.1, ed=0.3, es=0.7, a=0.1, d=0.3, s=0.9, r=0.25, mono=1)),
], [
    ('Bright', dict(cut=mul(1.7), treb=3)),
    ('Mellow', dict(cut=mul(0.6), fe=mul(0.7))),
    ('Soft', dict(a=mul(2.0), fe=mul(0.5))),
    ('Hard', dict(a=mul(0.4), fe=add(0.15), dist=0.2, dty=2)),
    ('Wide', dict(st=1.0, cho=add(0.25), v1=add(1))),
    ('Fall', dict(lt=T_PITCH, lw=3, lr=0.5, la=0.02, r=mul(1.6))),
    ('Big', dict(bass=3, rev=add(0.2), v1=add(1))),
    ('Tight', dict(d=mul(0.5), s=mul(0.5), r=mul(0.5))),
    ('Vintage', dict(d1=add(0.1), phs=0.2, cut=mul(0.85))),
    ('Swell', dict(a=mul(3.0), r=mul(1.5))),
]))

CATS.append(('Bell', [
    ('Glass Bell', dict(w1=SINE, v1=1, w2=SINE, v2=1, o2=2, s2=7, l2=0.35, cut=9000, a=0.001, d=1.8, s=0.0, r=1.5, rev=0.45, rsz=0.8)),
    ('Tubular', dict(w1=SINE, v1=1, w2=TRI, v2=1, o2=1, s2=3, l2=0.3, cut=6000, a=0.001, d=2.4, s=0.0, r=2.0, rev=0.4, rsz=0.9)),
    ('Music Box', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=2, l2=0.35, cut=7000, a=0.001, d=0.9, s=0.0, r=0.8, dly=0.3, rev=0.3)),
    ('Vibes', dict(w1=SINE, v1=1, w2=SINE, v2=1, o2=2, l2=0.25, cut=6500, a=0.001, d=1.5, s=0.0, r=1.0, lt=T_AMP, lw=0, lr=5.0, la=0.2, rev=0.3)),
    ('Chime', dict(w1=SINE, v1=2, d1=0.1, w2=SINE, v2=1, o2=3, s2=4, l2=0.3, cut=12000, a=0.001, d=2.8, s=0.0, r=2.4, rev=0.5, rsz=0.9, treb=3)),
    ('FM Bell', dict(w1=SINE, v1=1, w2=QUART, v2=1, o2=2, s2=5, l2=0.3, cut=8000, res=0.1, a=0.001, d=1.4, s=0.0, r=1.2, rev=0.4, cho=0.2)),
], [
    ('Bright', dict(cut=mul(1.6), treb=4)),
    ('Soft', dict(cut=mul(0.55))),
    ('Long', dict(d=mul(1.8), r=mul(1.8), rev=add(0.15))),
    ('Short', dict(d=mul(0.45), r=mul(0.5))),
    ('Wide', dict(st=1.0, cho=0.35, v1=add(1), d1=add(0.15))),
    ('Echo', dict(dly=0.4, dfb=0.5)),
    ('Dark', dict(cut=mul(0.4), bass=3)),
    ('Shimmer', dict(phs=0.4, cho=0.3, rev=add(0.2))),
    ('Cold', dict(treb=5, bass=-3, rsz=0.95)),
    ('Dry', dict(rev=0.0, dly=0.0)),
]))

CATS.append(('Choir', [
    ('Aah', dict(w1=SAW, v1=4, d1=0.15, w2=TRI, v2=2, l2=0.5, ft=1, cut=900, res=0.55, a=0.35, d=0.5, s=0.9, r=0.8, vib=0.012, rev=0.4, cho=0.25, st=0.9)),
    ('Ooh', dict(w1=TRI, v1=3, d1=0.12, w2=SINE, v2=1, l2=0.5, ft=1, cut=600, res=0.45, a=0.4, d=0.5, s=0.9, r=0.9, vib=0.012, rev=0.4, cho=0.25, st=0.8)),
    ('Voices', dict(w1=SAW, v1=5, d1=0.3, w2=PULSE, v2=2, l2=0.35, ft=1, cut=1100, res=0.5, a=0.3, d=0.6, s=0.85, r=0.9, vib=0.014, rev=0.45, cho=0.4, st=1.0)),
    ('Boys', dict(w1=SAW, v1=3, d1=0.12, o1=1, w2=TRI, v2=1, o2=1, l2=0.4, ft=1, cut=1400, res=0.45, a=0.2, d=0.5, s=0.85, r=0.6, vib=0.012, rev=0.35, cho=0.2)),
    ('Vocoder', dict(w1=SAW, v1=3, d1=0.2, w2=NOISE, v2=1, l2=0.08, ft=1, cut=1300, res=0.65, lt=T_CUT, lw=8, lr=3.0, la=0.3, a=0.05, d=0.4, s=0.85, r=0.25, cho=0.3)),
    ('Angels', dict(w1=TRI, v1=4, d1=0.2, w2=SINE, v2=2, o2=1, l2=0.4, ft=1, cut=1700, res=0.4, a=0.9, d=0.7, s=0.85, r=1.6, vib=0.01, rev=0.6, rsz=0.9, cho=0.3, st=1.0)),
], [
    ('Soft', dict(a=mul(1.8), cut=mul(0.8))),
    ('Bright', dict(cut=mul(1.5), treb=3)),
    ('Dark', dict(cut=mul(0.65), bass=3)),
    ('Wide', dict(st=1.0, cho=add(0.2), v1=add(1))),
    ('Hall', dict(rev=0.65, rsz=0.95)),
    ('Breathy', dict(w2=NOISE, v2=1, l2=0.06)),
    ('Slow', dict(a=mul(2.2), r=mul(1.6))),
    ('Choral', dict(cho=0.55, phs=0.2)),
    ('Distant', dict(rev=0.7, treb=-4, cut=mul(0.7))),
    ('Singing', dict(vib=0.025)),
]))

CATS.append(('Ambient', [
    ('Drift', dict(w1=SAW, v1=3, d1=0.35, w2=TRI, v2=2, o2=1, l2=0.4, cut=1800, res=0.2, lt=T_CUT, lw=8, lr=0.15, la=0.3, a=2.5, d=1.5, s=0.85, r=3.5, cho=0.4, rev=0.6, rsz=0.95, dly=0.3, st=1.0)),
    ('Space', dict(w1=SINE, v1=3, d1=0.3, w2=TRI, v2=2, o2=1, l2=0.4, cut=4000, a=2.0, d=1.5, s=0.9, r=4.0, phs=0.4, rev=0.7, rsz=1.0, dly=0.35, st=1.0)),
    ('Texture', dict(w1=SAW, v1=2, d1=0.5, w2=NOISE, v2=1, l2=0.12, cut=2200, res=0.3, lt=T_CUT, lw=7, lr=0.8, la=0.3, a=1.6, d=1.0, s=0.8, r=2.5, cho=0.3, rev=0.55, st=1.0)),
    ('Shimmer', dict(w1=TRI, v1=4, d1=0.3, w2=SINE, v2=2, o2=2, l2=0.3, cut=8000, a=1.5, d=1.2, s=0.85, r=3.0, phs=0.35, cho=0.4, rev=0.65, dly=0.4, st=1.0)),
    ('Dark Drone', dict(w1=SAW, v1=3, d1=0.25, o1=-1, w2=PULSE, v2=1, o2=-1, l2=0.4, cut=700, res=0.3, lt=T_CUT, lw=0, lr=0.07, la=0.35, a=3.0, d=2.0, s=0.9, r=4.0, rev=0.55, bass=3, st=0.8)),
    ('Awakening', dict(w1=TRISAW, v1=4, d1=0.3, cut=600, res=0.25, fe=0.5, ea=3.0, ed=2.0, es=0.5, er=3.0, a=2.5, d=1.5, s=0.85, r=3.5, cho=0.35, rev=0.6, dly=0.3, st=1.0)),
], [
    ('Slow', dict(a=mul(1.6), r=mul(1.5), lr=mul(0.6))),
    ('Bright', dict(cut=mul(1.8), treb=3)),
    ('Deep', dict(cut=mul(0.55), bass=4)),
    ('Frozen', dict(rev=0.8, rsz=1.0, treb=-2, dly=add(0.1))),
    ('Rolling', dict(lt=T_DETUNE, lw=8, lr=0.25, la=0.35)),
    ('Warm', dict(cut=mul(0.8), bass=2, cho=add(0.15))),
    ('Cold', dict(treb=4, bass=-3, cut=mul(1.3))),
    ('Haze', dict(phs=0.4, rev=add(0.15))),
    ('Lonely', dict(v1=1, v2=0, st=0.6, dly=0.4)),
    ('Vast', dict(rev=0.75, rsz=1.0, v1=add(1), st=1.0)),
]))

CATS.append(('FX', [
    ('Riser', dict(w1=NOISE, v1=1, w2=SAW, v2=3, d1=0.4, l2=0.3, ft=1, cut=300, res=0.45, fe=0.7, ea=3.0, ed=2.0, es=1.0, er=1.0, a=3.0, d=1.0, s=1.0, r=0.6, rev=0.35, dly=0.2, st=1.0)),
    ('Downlifter', dict(w1=NOISE, v1=1, w2=SAW, v2=3, d1=0.4, l2=0.3, ft=1, cut=9000, res=0.4, fe=-0.7, ea=0.01, ed=2.5, es=0.0, er=1.5, a=0.01, d=2.5, s=0.0, r=1.5, rev=0.4, st=1.0)),
    ('Laser', dict(w1=SAW, v1=1, w2=PULSE, v2=1, l2=0.4, cut=6000, res=0.4, lt=T_PITCH, lw=3, lr=3.0, la=0.5, a=0.001, d=0.4, s=0.0, r=0.2, dly=0.3)),
    ('Wind', dict(w1=NOISE, v1=1, ft=1, cut=800, res=0.5, lt=T_CUT, lw=8, lr=0.3, la=0.4, a=1.5, d=1.0, s=0.9, r=2.0, rev=0.5, st=1.0)),
    ('Robot', dict(w1=PULSE, v1=2, d1=0.5, w2=QUART, v2=1, l2=0.5, ft=1, cut=1500, res=0.55, lt=T_CUT, lw=9, lr=6.0, la=0.4, a=0.002, d=0.3, s=0.6, r=0.2, dist=0.45, dty=3, dly=0.2)),
    ('Impact', dict(w1=SINE, v1=1, o1=-2, w2=NOISE, v2=1, l2=0.25, cut=1800, res=0.2, lt=T_PITCH, lw=3, lr=1.2, la=0.25, a=0.001, d=1.2, s=0.0, r=1.0, rev=0.5, rsz=0.9, comp=1, bass=5)),
], [
    ('Short', dict(a=mul(0.5), d=mul(0.5), r=mul(0.5))),
    ('Long', dict(a=mul(1.8), d=mul(1.8), r=mul(1.8), rev=add(0.2))),
    ('Dirty', dict(dist=add(0.4), drv=add(0.2), dty=4)),
    ('Wide', dict(st=1.0, cho=0.4, phs=0.3)),
    ('Dark', dict(cut=mul(0.5), treb=-4, bass=3)),
    ('Bright', dict(cut=mul(1.8), treb=4)),
    ('Echo', dict(dly=0.5, dfb=0.55)),
    ('Wet', dict(rev=0.65, rsz=0.95)),
    ('Wobble', dict(lt=T_CUT, lw=0, lr=4.0, la=0.45)),
    ('Phased', dict(phs=0.5, cho=0.2)),
]))


def build():
    rows = []
    seen = set()
    sounds = {}
    for cat, archs, adjs in CATS:
        for k, (aname, amods) in enumerate(archs):
            rec = apply(BASE, {m: v for m, v in amods.items() if m in BASE})
            picks = []
            idx = (k * 3) % len(adjs)
            while len(picks) < 5:
                if adjs[idx][0] not in [p[0] for p in picks]:
                    picks.append(adjs[idx])
                idx = (idx + 1 + (len(picks) % 2)) % len(adjs)
            for word, wmods in picks:
                name = f'{aname} {word}'
                assert name not in seen, name
                seen.add(name)
                r = apply(rec, wmods)
                c = clean(r)
                key = tuple(sorted(c.items()))
                assert key not in sounds, f'{name} sounds like {sounds[key]}'
                sounds[key] = name
                rows.append((cat, name, c))
    return rows


def emit(rows):
    cats = [c[0] for c in CATS]
    out = ['// GENERATED by tools/gen_rind_presets.py from its recipes: edit the script and run it again.', '',
           'use crate::presets::{BASE, Preset, Recipe};', '',
           'pub const CATEGORIES: [&str; %d] = [%s];' % (len(cats) + 1, ', '.join(['"Init"'] + [f'"{c}"' for c in cats])), '',
           'pub const TABLE: &[Preset] = &[',
           '    Preset { cat: 0, name: "Init Saw", recipe: BASE },']
    for cat, name, rec in rows:
        fields = ', '.join(f'{k}: {rec[k]}' for k in rec)
        ci = cats.index(cat) + 1
        r = f'Recipe {{ {fields}, ..BASE }}' if fields else 'BASE'
        out.append(f'    Preset {{ cat: {ci}, name: "{name}", recipe: {r} }},')
    out.append('];')
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    rows = build()
    text = emit(rows)
    # Rust floats need a point; ints in float fields are written with one.
    def fix(m):
        key, val = m.group(1), m.group(2)
        return f'{key}: {val}.0' if key not in INT and key != 'cat' and '.' not in val and 'e' not in val else m.group(0)
    text = re.sub(r'\b([a-z0-9]+): (-?\d+)(?=[,\s])', fix, text)
    open('crates/rind/src/preset_table.rs', 'w').write(text)
    import subprocess
    subprocess.run(['rustfmt', '--edition', '2024', 'crates/rind/src/preset_table.rs'], check=True)
    print(len(rows) + 1, 'patches')
