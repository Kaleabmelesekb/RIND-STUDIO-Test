"""The genre categories of Rind's factory patches, for gen_rind_presets.py.

A genre is eight archetypes; each archetype has a kind (bass, lead, pad, pluck, keys, arp, fx) that supplies its plain
defaults, and is then built in a dozen flavours drawn from the flavours that suit its kind (see FLAVOURS). Waves and
filters use the engine's appended parts: PWM, Saw+Sub, Pulse+Sub and SuperSaw oscillators, and the ladder filters.
"""

SINE, SAW, TRI, PULSE, HALF, QUART, TRISAW, NOISE, PWM, SAWSUB, PLSUB, SUPER = range(12)
LAD, LADBP, LADHP = 3, 4, 5
T_CUT, T_RES, T_PITCH, T_DETUNE, T_AMP, T_PAN, T_LVL2, T_W1, T_WALL = 1, 3, 7, 14, 19, 32, 31, 35, 39


def mul(x): return ('*', x)
def add(x): return ('+', x)


KIND = dict(
    bass=dict(mono=1, o1=-1, v1=1, cut=700, res=0.2, a=0.002, d=0.4, s=0.8, r=0.12),
    lead=dict(mono=1, v1=3, d1=0.2, cut=5000, res=0.2, a=0.004, d=0.4, s=0.85, r=0.25, dly=0.2, rev=0.2),
    pad=dict(v1=4, d1=0.3, cut=2500, a=0.8, d=0.8, s=0.85, r=1.5, cho=0.35, rev=0.35, st=0.9),
    pluck=dict(v1=2, d1=0.15, cut=3000, res=0.25, fe=0.4, ed=0.18, es=0.0, a=0.001, d=0.3, s=0.0, r=0.25, rev=0.2),
    keys=dict(v1=1, cut=4000, a=0.003, d=0.9, s=0.3, r=0.4, rev=0.2),
    arp=dict(v1=2, d1=0.2, cut=3500, res=0.25, a=0.002, d=0.25, s=0.4, r=0.15, arp=1, div=11, aoct=2, gate=0.5, dly=0.3, rev=0.2),
    fx=dict(cut=3000, a=0.5, d=1.0, s=0.7, r=1.0, rev=0.4, st=1.0),
)

# ---- flavours: (word, mods), grouped by what suits a kind -------------------------------------------------------
F = {}
F['Dark'] = dict(cut=mul(0.55), treb=-3)
F['Bright'] = dict(cut=mul(1.8), treb=3)
F['Wide'] = dict(st=1.0, cho=add(0.25), v1=add(1))
F['Narrow'] = dict(st=0.25, cho=0.0)
F['Tight'] = dict(d=mul(0.5), r=mul(0.5))
F['Long'] = dict(d=mul(1.8), r=mul(2.0), rev=add(0.2))
F['Dirty'] = dict(dist=add(0.3), drv=add(0.2), dty=1)
F['Crunch'] = dict(dist=0.5, dty=3, res=add(0.1))
F['Clean'] = dict(dist=0.0, drv=0.0, cho=0.0)
F['Echo'] = dict(dly=0.4, dfb=0.5)
F['Wet'] = dict(rev=0.55, rsz=0.9)
F['Dry'] = dict(rev=0.0, dly=0.0)
F['Phased'] = dict(phs=0.4)
F['Vintage'] = dict(d1=add(0.1), phs=0.2, cut=mul(0.8), treb=-2)
F['Octave Up'] = dict(o2=1, v2=1, l2=0.4)
F['Octave Down'] = dict(o2=-1, v2=1, l2=0.5)
F['Fifth'] = dict(s2=7, v2=1, l2=0.35)
F['Sub'] = dict(w2=SINE, o2=-1, v2=1, l2=0.6)
F['Ladder'] = dict(ft=LAD, res=add(0.15))
F['Ladder Bite'] = dict(ft=LAD, res=0.55, drv=0.3)
F['Pulse Width'] = dict(w1=PWM, lt=T_W1, lw=1, lr=0.4, la=0.55)
F['Super'] = dict(w1=SUPER, v1=7, d1=0.45)
F['Sub Osc'] = dict(w1=SAWSUB)
F['Resonant'] = dict(res=add(0.3))
F['Open'] = dict(fe=add(0.3), ea=0.003)
F['Wobble'] = dict(lt=T_CUT, lw=0, lr=3.0, la=0.4)
F['Pump'] = dict(lt=T_AMP, lw=4, lr=2.0, la=0.5)
F['Slow Sweep'] = dict(lt=T_CUT, lw=1, lr=0.2, la=0.4)
F['Glide'] = dict(mono=1, porta=0.1)
F['Gritty'] = dict(w2=NOISE, v2=1, l2=0.05)
F['Vibrato'] = dict(vib=0.02)
F['Soft'] = dict(a=add(0.03), cut=mul(0.7))
F['Punchy'] = dict(comp=1, ed=mul(0.5))
F['Hollow'] = dict(w1=HALF)
F['Fat'] = dict(v1=add(2), d1=add(0.15), bass=4)
F['Airy'] = dict(treb=4, rev=0.4, cut=mul(1.4))
F['Detuned'] = dict(d1=add(0.3))
F['Sweep'] = dict(ft=LADBP, res=0.4, lt=T_CUT, lw=1, lr=0.35, la=0.5)
F['Hi-Pass'] = dict(ft=LADHP, cut=mul(0.6), res=0.2)

BY_KIND = dict(
    bass=['Dark', 'Bright', 'Tight', 'Dirty', 'Crunch', 'Clean', 'Ladder', 'Ladder Bite', 'Pulse Width', 'Sub', 'Sub Osc',
          'Resonant', 'Wobble', 'Pump', 'Glide', 'Gritty', 'Punchy', 'Fat', 'Detuned', 'Open', 'Hollow', 'Octave Up',
          'Narrow', 'Long', 'Vintage', 'Wet'],
    lead=['Dark', 'Bright', 'Wide', 'Tight', 'Long', 'Dirty', 'Echo', 'Wet', 'Phased', 'Vintage', 'Octave Up',
          'Octave Down', 'Fifth', 'Ladder', 'Pulse Width', 'Super', 'Resonant', 'Open', 'Glide', 'Vibrato', 'Gritty', 'Fat',
          'Airy', 'Detuned', 'Crunch', 'Soft'],
    pad=['Dark', 'Bright', 'Wide', 'Long', 'Wet', 'Phased', 'Vintage', 'Octave Up', 'Fifth', 'Ladder', 'Pulse Width',
         'Super', 'Slow Sweep', 'Soft', 'Airy', 'Detuned', 'Gritty', 'Open', 'Hollow', 'Vibrato', 'Echo', 'Fat', 'Narrow',
         'Octave Down', 'Sweep', 'Hi-Pass'],
    pluck=['Dark', 'Bright', 'Wide', 'Tight', 'Long', 'Dirty', 'Echo', 'Wet', 'Dry', 'Phased', 'Vintage', 'Octave Up',
           'Fifth', 'Ladder', 'Pulse Width', 'Resonant', 'Open', 'Soft', 'Punchy', 'Clean', 'Airy', 'Detuned', 'Hollow',
           'Gritty', 'Super', 'Sub'],
    keys=['Dark', 'Bright', 'Wide', 'Tight', 'Long', 'Dirty', 'Echo', 'Wet', 'Dry', 'Phased', 'Vintage', 'Octave Up',
          'Fifth', 'Ladder', 'Pulse Width', 'Resonant', 'Open', 'Soft', 'Punchy', 'Clean', 'Airy', 'Detuned', 'Hollow',
          'Gritty', 'Sub', 'Vibrato'],
)
BY_KIND['arp'] = BY_KIND['pluck'] + ['Pump', 'Wobble', 'Super', 'Fat']
BY_KIND['fx'] = list(F)

# ---- the genres ------------------------------------------------------------------------------------------------
G = []


def genre(name, *archs):
    G.append((name, archs))


genre('Trance',
      ('Supersaw Lead', 'lead', dict(w1=SUPER, v1=7, d1=0.5, st=1.0, cut=7000, res=0.1, cho=0.2, dly=0.3, rev=0.3)),
      ('Uplift Chord', 'pad', dict(w1=SUPER, v1=8, d1=0.55, w2=SAW, v2=4, o2=1, l2=0.3, cut=5500, a=0.01, d=0.5, s=0.8, r=0.5, rev=0.35, dly=0.25)),
      ('Gated Pluck', 'pluck', dict(w1=SAW, v1=5, d1=0.3, cut=4500, res=0.2, fe=0.4, ed=0.12, d=0.2, r=0.2, dly=0.35, rev=0.25, st=0.9)),
      ('Rolling Bass', 'bass', dict(w1=SAWSUB, ft=LAD, cut=450, res=0.35, fe=0.45, ed=0.14, es=0.1, d=0.2, s=0.4, r=0.08, drv=0.2)),
      ('Acid Trance', 'arp', dict(w1=SAW, v1=1, mono=1, ft=LAD, cut=650, res=0.65, fe=0.55, ed=0.2, es=0.1, pat=1, porta=0.03, aoct=1, gate=0.6, dly=0.3)),
      ('Pwm Arp', 'arp', dict(w1=PWM, lt=T_W1, lw=1, lr=0.3, la=0.5, v1=2, d1=0.2, cut=5000, aoct=2, gate=0.45, dly=0.4, rev=0.25)),
      ('Breakdown Pad', 'pad', dict(w1=SAW, v1=6, d1=0.4, w2=TRI, v2=2, l2=0.4, ft=LAD, cut=1800, a=1.2, d=1.0, s=0.85, r=2.0, rev=0.5, st=1.0)),
      ('Noise Riser', 'fx', dict(w1=NOISE, v1=1, w2=SUPER, v2=5, d2=0.5, l2=0.3, ft=LAD, cut=300, res=0.4, fe=0.8, ea=4.0, ed=2.0, es=1.0, a=4.0, s=1.0, rev=0.4, dly=0.25)))

genre('House',
      ('Piano Stab', 'keys', dict(w1=PULSE, v1=2, d1=0.12, w2=SAW, v2=1, l2=0.3, cut=2800, res=0.15, fe=0.3, ed=0.2, es=0.1, d=0.35, s=0.0, r=0.2, cho=0.2, rev=0.2)),
      ('Organ Stab', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.7, cut=6000, a=0.002, d=0.12, s=0.0, r=0.08, lt=T_AMP, lw=0, lr=6.0, la=0.05, rev=0.15)),
      ('Deep Bass', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.3, cut=500, res=0.1, d=0.3, s=0.7, r=0.15, comp=1)),
      ('Offbeat Bass', 'bass', dict(w1=SAW, ft=LAD, cut=380, res=0.3, fe=0.4, ed=0.1, es=0.0, d=0.12, s=0.0, r=0.06, drv=0.15, comp=1)),
      ('Chord Hit', 'pluck', dict(w1=SAW, v1=4, d1=0.3, w2=PULSE, v2=2, l2=0.4, cut=3200, fe=0.5, ed=0.2, d=0.3, r=0.2, cho=0.3, dly=0.25, st=1.0)),
      ('French Pad', 'pad', dict(w1=SAW, v1=4, d1=0.25, ft=LAD, cut=1400, res=0.3, lt=T_CUT, lw=1, lr=0.25, la=0.3, a=0.4, r=1.0, phs=0.3, rev=0.4)),
      ('Vocal Chop', 'lead', dict(w1=SAW, v1=3, w2=TRI, v2=1, l2=0.5, ft=1, cut=1100, res=0.55, mono=0, a=0.003, d=0.25, s=0.3, r=0.2, vib=0.01, dly=0.3)),
      ('Pluck Lead', 'lead', dict(w1=PWM, lt=T_W1, lw=1, lr=0.2, la=0.4, v1=2, cut=3500, fe=0.4, ed=0.2, es=0.3, d=0.3, s=0.4, r=0.2, dly=0.35)))

genre('Techno',
      ('Rumble Bass', 'bass', dict(w1=SAW, v1=2, d1=0.1, ft=LAD, cut=260, res=0.25, d=0.15, s=0.0, r=0.1, drv=0.4, dist=0.25, dty=1, rev=0.3)),
      ('Hoover', 'lead', dict(w1=SAW, v1=6, d1=0.6, w2=SAW, v2=3, o2=-1, l2=0.5, cut=3000, res=0.2, lt=T_PITCH, lw=3, lr=0.4, la=0.08, porta=0.15, a=0.01, d=0.3, s=0.9, r=0.3, dist=0.3)),
      ('Stab', 'pluck', dict(w1=PULSE, v1=3, d1=0.2, cut=1800, res=0.4, fe=0.5, ed=0.12, d=0.18, r=0.12, dly=0.4, dfb=0.55, rev=0.3)),
      ('Acid Line', 'arp', dict(w1=SAW, v1=1, mono=1, ft=LAD, cut=600, res=0.8, fe=0.6, ed=0.16, es=0.05, d=0.2, s=0.2, r=0.08, drv=0.45, dist=0.3, pat=1, aoct=1, gate=0.7, porta=0.05)),
      ('Industrial Hit', 'fx', dict(w1=NOISE, v1=1, w2=SAW, v2=2, l2=0.4, ft=LADBP, cut=800, res=0.5, a=0.001, d=0.6, s=0.0, r=0.5, dist=0.6, dty=3, rev=0.5, comp=1)),
      ('Dark Pad', 'pad', dict(w1=PULSE, v1=3, d1=0.35, ft=LAD, cut=700, res=0.3, lt=T_CUT, lw=8, lr=0.12, la=0.35, a=2.0, r=2.5, rev=0.55, st=0.8)),
      ('Metal Chord', 'pluck', dict(w1=SAW, v1=3, d1=0.2, w2=SAW, v2=1, s2=6, l2=0.5, ft=LAD, cut=1500, res=0.45, fe=0.4, ed=0.2, d=0.4, r=0.3, dly=0.35, rev=0.35)),
      ('Pulse Seq', 'arp', dict(w1=PWM, lt=T_W1, lw=0, lr=0.5, la=0.6, v1=1, cut=1600, res=0.5, fe=0.35, ed=0.14, es=0.1, d=0.18, s=0.0, pat=2, aoct=2, gate=0.4, dly=0.3)))

genre('Dubstep',
      ('Wobble Bass', 'bass', dict(w1=SAWSUB, v1=2, d1=0.25, ft=LAD, cut=500, res=0.55, lt=T_CUT, lw=0, lr=2.0, la=0.7, d=0.4, s=0.9, r=0.1, dist=0.3, dty=1, drv=0.3)),
      ('Growl', 'bass', dict(w1=PWM, lt=T_W1, lw=0, lr=4.0, la=0.6, v1=3, d1=0.3, w2=SAW, v2=2, l2=0.5, ft=LAD, cut=900, res=0.5, dist=0.5, dty=3, drv=0.4, s=0.9)),
      ('Yoy Bass', 'bass', dict(w1=SAW, v1=3, d1=0.3, ft=LADBP, cut=700, res=0.6, lt=T_CUT, lw=1, lr=1.2, la=0.8, dist=0.4, dty=1, s=0.9)),
      ('Screech', 'lead', dict(w1=SUPER, v1=5, d1=0.6, cut=8000, res=0.5, lt=T_PITCH, lw=3, lr=1.0, la=0.05, dist=0.5, dty=2, drv=0.3, dly=0.2, rev=0.3)),
      ('Supersaw Drop', 'pad', dict(w1=SUPER, v1=8, d1=0.65, w2=SAW, v2=4, o2=-1, l2=0.4, cut=4500, a=0.005, d=0.6, s=0.9, r=0.5, st=1.0, rev=0.3, dist=0.2, comp=1)),
      ('Sub Drop', 'bass', dict(w1=SINE, w2=SINE, v2=1, o2=-1, l2=0.6, cut=400, lt=T_PITCH, lw=3, lr=0.8, la=0.4, a=0.001, d=1.2, s=0.0, r=0.5, dist=0.1, bass=4)),
      ('Impact', 'fx', dict(w1=NOISE, v1=1, w2=SINE, v2=1, o2=-2, l2=0.7, cut=2200, res=0.2, a=0.001, d=1.6, s=0.0, r=1.4, rev=0.6, rsz=0.95, comp=1, bass=5)),
      ('Riser Siren', 'fx', dict(w1=SAW, v1=4, d1=0.4, ft=LADBP, cut=400, res=0.5, fe=0.7, ea=4.0, ed=2.0, es=1.0, lt=T_PITCH, lw=2, lr=0.2, la=0.5, a=4.0, s=1.0, rev=0.45)))

genre('Future Bass',
      ('Supersaw Chord', 'pad', dict(w1=SUPER, v1=8, d1=0.5, w2=SAW, v2=3, o2=1, l2=0.3, cut=3500, a=0.004, d=0.4, s=0.9, r=0.4, st=1.0, rev=0.35, lt=T_AMP, lw=4, lr=2.0, la=0.45, cho=0.3)),
      ('Wub Lead', 'lead', dict(w1=SUPER, v1=5, d1=0.4, ft=LAD, cut=1800, res=0.3, lt=T_CUT, lw=0, lr=3.0, la=0.5, mono=1, porta=0.04, rev=0.3, dly=0.3)),
      ('Pluck Chord', 'pluck', dict(w1=SAW, v1=6, d1=0.4, cut=4500, res=0.2, fe=0.45, ed=0.14, d=0.3, r=0.3, cho=0.3, dly=0.35, rev=0.35, st=1.0)),
      ('Sub Bass', 'bass', dict(w1=SINE, cut=500, d=0.3, s=0.9, r=0.18, porta=0.06, comp=1)),
      ('Bell Lead', 'lead', dict(w1=SINE, v1=2, d1=0.1, w2=TRI, v2=1, o2=2, l2=0.35, cut=8000, mono=0, a=0.001, d=0.5, s=0.1, r=0.5, dly=0.4, rev=0.4, cho=0.3)),
      ('Vocal Pad', 'pad', dict(w1=SAW, v1=5, d1=0.3, ft=1, cut=1200, res=0.5, a=0.2, r=0.8, vib=0.01, st=1.0, rev=0.4, cho=0.4)),
      ('Kawaii Arp', 'arp', dict(w1=TRI, v1=2, d1=0.15, w2=SINE, v2=1, o2=1, l2=0.4, cut=6000, aoct=2, gate=0.6, div=11, dly=0.4, rev=0.4, cho=0.3)),
      ('Glitch FX', 'fx', dict(w1=PULSE, v1=2, d1=0.4, ft=LADBP, cut=1500, res=0.5, lt=T_CUT, lw=7, lr=8.0, la=0.6, a=0.001, d=0.3, s=0.2, r=0.2, dly=0.4, rev=0.3)))

genre('Trap',
      ('808 Bass', 'bass', dict(w1=SINE, cut=900, res=0.05, a=0.001, d=1.5, s=0.0, r=0.4, porta=0.05, dist=0.2, dty=0, drv=0.2, bass=4, comp=1)),
      ('808 Glide', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.2, cut=800, lt=T_PITCH, lw=3, lr=1.5, la=0.12, a=0.001, d=1.8, s=0.0, r=0.5, dist=0.25, dty=1, comp=1)),
      ('Dark Pluck', 'pluck', dict(w1=SAW, v1=2, d1=0.15, ft=LAD, cut=1400, res=0.3, fe=0.4, ed=0.2, d=0.4, r=0.4, rev=0.4, dly=0.25, treb=-2)),
      ('Bell Melody', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=2, s2=7, l2=0.3, cut=7000, d=1.2, s=0.0, r=0.8, rev=0.5, dly=0.3)),
      ('Choir Pad', 'pad', dict(w1=TRI, v1=3, w2=SAW, v2=2, l2=0.3, ft=1, cut=900, res=0.4, a=0.3, r=1.2, vib=0.012, rev=0.6, st=1.0)),
      ('Flute Lead', 'lead', dict(w1=SINE, v1=1, w2=TRI, v2=1, o2=1, l2=0.3, cut=4000, a=0.04, d=0.3, s=0.8, r=0.3, vib=0.02, rev=0.4, dly=0.3)),
      ('Hi-Hat Noise', 'fx', dict(w1=NOISE, v1=1, ft=LADHP, cut=7000, res=0.1, a=0.001, d=0.06, s=0.0, r=0.04, rev=0.1)),
      ('Siren Lead', 'lead', dict(w1=PULSE, v1=2, d1=0.2, cut=3500, lt=T_PITCH, lw=1, lr=1.2, la=0.1, a=0.01, d=0.3, s=0.9, r=0.3, dist=0.2, rev=0.35)))

genre('DnB',
      ('Reese Bass', 'bass', dict(w1=SAW, v1=3, d1=0.4, w2=SAW, v2=3, d2=0.45, o2=-1, l2=0.8, ft=LAD, cut=600, res=0.2, lt=T_DETUNE, lw=1, lr=0.15, la=0.3, s=0.9, r=0.2, dist=0.25, cho=0.3, drv=0.2)),
      ('Neuro Bass', 'bass', dict(w1=PWM, lt=T_W1, lw=1, lr=1.5, la=0.6, v1=3, d1=0.3, ft=LADBP, cut=900, res=0.55, fe=0.3, ed=0.2, es=0.6, dist=0.5, dty=3, drv=0.35, s=0.9)),
      ('Jump Bass', 'bass', dict(w1=SAWSUB, v1=2, d1=0.2, cut=800, res=0.3, fe=0.35, ed=0.12, es=0.3, d=0.2, s=0.5, r=0.1, drv=0.2)),
      ('Liquid Pad', 'pad', dict(w1=SAW, v1=5, d1=0.3, w2=TRI, v2=2, l2=0.4, cut=3500, a=0.6, d=0.8, s=0.85, r=1.2, cho=0.45, rev=0.5, st=1.0)),
      ('Liquid Keys', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=1, l2=0.25, cut=4500, fe=0.15, d=1.0, s=0.15, r=0.6, cho=0.3, rev=0.35, phs=0.2)),
      ('Amen Stab', 'pluck', dict(w1=SAW, v1=3, d1=0.25, cut=2600, res=0.3, fe=0.45, ed=0.14, d=0.22, r=0.14, dly=0.3, dfb=0.5, rev=0.3)),
      ('Atmos Texture', 'fx', dict(w1=NOISE, v1=1, w2=SAW, v2=3, d2=0.5, l2=0.3, ft=1, cut=1500, res=0.3, lt=T_CUT, lw=8, lr=0.3, la=0.4, a=2.0, r=3.0, rev=0.7, st=1.0)),
      ('Rolling Arp', 'arp', dict(w1=SAW, v1=2, d1=0.2, ft=LAD, cut=2200, res=0.3, fe=0.3, ed=0.14, es=0.2, div=12, aoct=2, gate=0.45, pat=3, dly=0.3, rev=0.25)))

genre('Hardstyle',
      ('Kick Bass', 'bass', dict(w1=SINE, w2=SAW, v2=1, l2=0.3, cut=1200, lt=T_PITCH, lw=3, lr=3.0, la=0.18, d=0.25, s=0.0, r=0.12, dist=0.6, dty=2, drv=0.5, comp=1)),
      ('Screech Lead', 'lead', dict(w1=SUPER, v1=6, d1=0.45, w2=SAW, v2=2, o2=1, l2=0.4, cut=7500, res=0.35, dist=0.55, dty=2, drv=0.3, dly=0.2, rev=0.35)),
      ('Euphoric Lead', 'lead', dict(w1=SUPER, v1=8, d1=0.55, st=1.0, cut=8000, res=0.1, a=0.004, s=0.95, r=0.35, rev=0.4, dly=0.35, cho=0.25)),
      ('Hard Stab', 'pluck', dict(w1=SAW, v1=4, d1=0.3, w2=PULSE, v2=2, l2=0.4, cut=3000, res=0.3, fe=0.4, ed=0.1, d=0.16, r=0.1, dist=0.4, dty=1, rev=0.2, comp=1)),
      ('Rawstyle Bass', 'bass', dict(w1=PULSE, v1=2, d1=0.25, ft=LAD, cut=700, res=0.4, lt=T_CUT, lw=3, lr=3.0, la=0.5, d=0.3, s=0.7, dist=0.6, dty=3, drv=0.5)),
      ('Anthem Pad', 'pad', dict(w1=SUPER, v1=8, d1=0.5, cut=5500, a=0.4, s=0.9, r=1.2, st=1.0, rev=0.5, cho=0.35)),
      ('Reverse Bass', 'fx', dict(w1=SAW, v1=3, d1=0.3, ft=LAD, cut=200, res=0.3, fe=0.8, ea=0.5, ed=0.2, es=0.0, lt=T_AMP, lw=4, lr=8.0, la=0.5, a=0.01, d=0.4, s=0.0, dist=0.4, comp=1)),
      ('Impact Hit', 'fx', dict(w1=NOISE, v1=1, w2=SINE, v2=1, o2=-2, l2=0.6, cut=3000, a=0.001, d=1.0, s=0.0, r=0.8, rev=0.55, comp=1, bass=5, dist=0.3)))

genre('Synthwave',
      ('Retro Lead', 'lead', dict(w1=SAW, v1=3, d1=0.2, w2=PULSE, v2=1, l2=0.4, ft=LAD, cut=3600, res=0.25, vib=0.012, dly=0.4, dfb=0.45, rev=0.35, cho=0.3)),
      ('Outrun Bass', 'bass', dict(w1=SAWSUB, ft=LAD, cut=900, res=0.3, fe=0.3, ed=0.15, es=0.2, d=0.18, s=0.3, r=0.08, drv=0.2, arp=1, div=11, aoct=1, gate=0.55)),
      ('Night Pad', 'pad', dict(w1=SAW, v1=6, d1=0.35, w2=PULSE, v2=2, l2=0.4, ft=LAD, cut=2200, a=0.9, r=2.0, cho=0.5, phs=0.3, rev=0.5, st=1.0)),
      ('Neon Arp', 'arp', dict(w1=PWM, lt=T_W1, lw=1, lr=0.4, la=0.5, v1=2, d1=0.2, ft=LAD, cut=3800, res=0.3, aoct=2, gate=0.5, dly=0.4, dfb=0.5, rev=0.3)),
      ('Poly Brass', 'pad', dict(w1=SAW, v1=5, d1=0.25, ft=LAD, cut=1400, res=0.2, fe=0.5, ea=0.06, ed=0.3, es=0.6, a=0.05, d=0.4, s=0.8, r=0.3, cho=0.3, rev=0.3)),
      ('Gated Snare Synth', 'fx', dict(w1=NOISE, v1=1, w2=TRI, v2=1, o2=-1, l2=0.4, ft=LADBP, cut=1800, res=0.2, a=0.001, d=0.3, s=0.0, r=0.3, rev=0.6, rsz=0.8, comp=1)),
      ('Electric Keys', 'keys', dict(w1=TRI, w2=SINE, v2=1, o2=1, l2=0.3, cut=3500, fe=0.2, d=0.8, s=0.3, r=0.4, cho=0.4, phs=0.25, rev=0.3)),
      ('Sunset Pluck', 'pluck', dict(w1=SAW, v1=3, d1=0.2, ft=LAD, cut=2400, res=0.3, fe=0.4, ed=0.2, d=0.35, r=0.35, dly=0.4, rev=0.4, cho=0.3)))

genre('Electro',
      ('Electro Bass', 'bass', dict(w1=SAW, ft=LAD, cut=420, res=0.4, fe=0.45, ed=0.12, es=0.15, d=0.18, s=0.3, r=0.08, drv=0.3, dist=0.15, comp=1)),
      ('Robot Lead', 'lead', dict(w1=PULSE, v1=2, d1=0.3, w2=QUART, v2=1, l2=0.4, ft=1, cut=1700, res=0.55, lt=T_CUT, lw=9, lr=5.0, la=0.35, dist=0.35, dty=3, dly=0.25)),
      ('Vocoder Pad', 'pad', dict(w1=SAW, v1=4, d1=0.25, w2=NOISE, v2=1, l2=0.08, ft=1, cut=1300, res=0.65, lt=T_CUT, lw=8, lr=2.5, la=0.3, a=0.1, r=0.5, cho=0.35)),
      ('Zap', 'fx', dict(w1=SAW, v1=1, w2=PULSE, v2=1, l2=0.4, cut=6000, res=0.4, lt=T_PITCH, lw=3, lr=4.0, la=0.5, a=0.001, d=0.25, s=0.0, r=0.1, dly=0.3)),
      ('Funk Stab', 'pluck', dict(w1=PULSE, v1=2, d1=0.15, cut=2500, res=0.35, fe=0.4, ed=0.12, d=0.2, r=0.12, cho=0.2, rev=0.15)),
      ('Arp Seq', 'arp', dict(w1=SAW, v1=2, d1=0.15, ft=LAD, cut=1800, res=0.45, fe=0.35, ed=0.14, es=0.1, pat=4, aoct=2, gate=0.4, dly=0.3)),
      ('Kraft Lead', 'lead', dict(w1=TRI, v1=1, w2=SAW, v2=1, l2=0.3, cut=3000, mono=1, porta=0.06, vib=0.015, a=0.01, s=0.85, dly=0.35, rev=0.3)),
      ('Pulse Pad', 'pad', dict(w1=PWM, lt=T_W1, lw=1, lr=0.2, la=0.5, v1=4, d1=0.25, cut=2000, a=0.5, r=1.2, cho=0.4, rev=0.4)))

genre('Chillout',
      ('Mellow Keys', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=1, l2=0.3, cut=3000, fe=0.1, d=1.4, s=0.2, r=0.8, cho=0.35, phs=0.2, rev=0.45)),
      ('Warm Pad', 'pad', dict(w1=SAW, v1=4, d1=0.25, w2=TRI, v2=2, l2=0.5, ft=LAD, cut=1600, a=1.2, r=2.2, cho=0.45, rev=0.5, st=1.0)),
      ('Soft Lead', 'lead', dict(w1=TRI, v1=2, d1=0.1, w2=SINE, v2=1, o2=1, l2=0.3, cut=3200, a=0.05, s=0.9, r=0.4, vib=0.015, dly=0.4, rev=0.45)),
      ('Sub Bass', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.15, cut=450, a=0.01, d=0.5, s=0.9, r=0.2, porta=0.05)),
      ('Pluck Harp', 'pluck', dict(w1=TRI, v1=2, d1=0.1, w2=SINE, v2=1, o2=2, l2=0.3, cut=4500, d=1.0, r=0.9, dly=0.4, rev=0.5)),
      ('Shimmer Bell', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=2, s2=7, l2=0.3, cut=8000, d=2.0, s=0.0, r=1.5, phs=0.35, rev=0.6, rsz=0.9)),
      ('Dream Pad', 'pad', dict(w1=TRI, v1=5, d1=0.3, w2=SINE, v2=2, o2=1, l2=0.4, cut=4500, a=1.8, d=1.2, r=3.0, phs=0.4, rev=0.65, rsz=0.95, st=1.0)),
      ('Beach Arp', 'arp', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=1, l2=0.35, cut=4500, aoct=2, gate=0.65, div=8, dly=0.45, dfb=0.5, rev=0.45)))

genre('Lo-Fi',
      ('Dusty Keys', 'keys', dict(w1=TRI, w2=SINE, v2=1, o2=1, l2=0.25, cut=2200, d=1.0, s=0.2, r=0.5, lt=T_PITCH, lw=8, lr=0.5, la=0.012, dist=0.15, treb=-4, cho=0.3, phs=0.15, rev=0.3)),
      ('Tape Pad', 'pad', dict(w1=SAW, v1=3, d1=0.3, ft=LAD, cut=1100, a=0.8, r=1.6, lt=T_PITCH, lw=8, lr=0.4, la=0.015, treb=-5, cho=0.4, rev=0.4, dist=0.1)),
      ('Mellow Bass', 'bass', dict(w1=TRI, cut=500, d=0.3, s=0.8, r=0.15, treb=-3, comp=1)),
      ('Vinyl Lead', 'lead', dict(w1=PULSE, v1=1, w2=SINE, v2=1, l2=0.3, cut=2400, a=0.01, s=0.85, r=0.3, vib=0.02, treb=-3, dly=0.35, rev=0.3, dist=0.1)),
      ('Muffled Pluck', 'pluck', dict(w1=TRI, v1=1, cut=1500, res=0.2, fe=0.3, d=0.4, r=0.4, treb=-4, rev=0.3, dly=0.3)),
      ('Wonky Chord', 'pad', dict(w1=SAW, v1=3, d1=0.2, w2=PULSE, v2=1, l2=0.3, cut=1400, a=0.05, d=0.6, s=0.5, r=0.5, lt=T_PITCH, lw=8, lr=0.6, la=0.02, treb=-4, cho=0.35)),
      ('Bit Bell', 'keys', dict(w1=QUART, w2=SINE, v2=1, o2=2, l2=0.25, cut=3500, d=0.9, s=0.0, r=0.7, treb=-2, rev=0.4, dist=0.2, dty=4)),
      ('Crackle Texture', 'fx', dict(w1=NOISE, v1=1, ft=LAD, cut=2500, res=0.2, lt=T_CUT, lw=7, lr=6.0, la=0.5, a=0.5, s=0.5, r=0.8, treb=-3, rev=0.3)))

genre('Hip-Hop',
      ('Boom Bass', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.15, cut=700, a=0.003, d=0.6, s=0.3, r=0.2, dist=0.15, bass=3, comp=1)),
      ('Soul Keys', 'keys', dict(w1=TRI, w2=SINE, v2=1, o2=1, l2=0.3, cut=2800, d=1.2, s=0.25, r=0.5, cho=0.3, phs=0.2, rev=0.25, dist=0.1)),
      ('Horn Stab', 'pluck', dict(w1=SAW, v1=3, d1=0.15, ft=LAD, cut=1300, res=0.2, fe=0.5, ea=0.03, ed=0.18, d=0.22, r=0.15, rev=0.3)),
      ('String Sample', 'pad', dict(w1=SAW, v1=4, d1=0.25, cut=2600, a=0.3, d=0.6, s=0.8, r=0.7, vib=0.01, cho=0.4, rev=0.35, treb=-3, st=1.0)),
      ('Piano Loop', 'keys', dict(w1=TRI, v1=2, d1=0.06, w2=SINE, v2=1, o2=1, l2=0.3, cut=3000, fe=0.2, d=1.6, s=0.1, r=0.7, rev=0.35)),
      ('Whistle Lead', 'lead', dict(w1=SINE, v1=1, w2=TRI, v2=1, o2=1, l2=0.15, cut=5000, a=0.03, s=0.9, r=0.3, vib=0.025, porta=0.05, dly=0.3, rev=0.35)),
      ('Dirty South Synth', 'lead', dict(w1=PULSE, v1=3, d1=0.25, cut=2600, res=0.3, a=0.003, d=0.3, s=0.8, r=0.2, dist=0.3, dty=1, rev=0.2)),
      ('Vinyl Pad', 'pad', dict(w1=TRI, v1=3, d1=0.2, ft=LAD, cut=1200, a=0.7, r=1.5, treb=-4, cho=0.35, rev=0.4)))

genre('R&B',
      ('Smooth Keys', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=2, l2=0.2, cut=4000, fe=0.15, d=1.5, s=0.2, r=0.7, cho=0.35, phs=0.2, rev=0.35)),
      ('Velvet Pad', 'pad', dict(w1=SAW, v1=5, d1=0.2, w2=TRI, v2=2, l2=0.5, ft=LAD, cut=1800, a=0.7, r=1.8, cho=0.5, rev=0.45, st=1.0)),
      ('Silky Lead', 'lead', dict(w1=TRI, v1=2, d1=0.1, w2=SINE, v2=1, o2=1, l2=0.35, cut=3800, a=0.04, s=0.9, r=0.4, vib=0.02, porta=0.06, dly=0.35, rev=0.4)),
      ('Round Bass', 'bass', dict(w1=SINE, w2=SAW, v2=1, l2=0.15, cut=650, d=0.5, s=0.6, r=0.2, porta=0.05, comp=1)),
      ('Rhodes', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=2, l2=0.3, cut=5000, fe=0.1, d=1.4, s=0.25, r=0.6, lt=T_AMP, lw=0, lr=4.0, la=0.12, cho=0.3)),
      ('Stacked Vox', 'pad', dict(w1=SAW, v1=5, d1=0.25, ft=1, cut=1000, res=0.5, a=0.3, r=1.0, vib=0.012, cho=0.35, rev=0.45, st=1.0)),
      ('Finger Snap Synth', 'pluck', dict(w1=NOISE, v1=1, w2=TRI, v2=1, o2=1, l2=0.4, ft=LADBP, cut=2000, res=0.3, d=0.12, r=0.08, rev=0.3)),
      ('Slow Jam Arp', 'arp', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=1, l2=0.3, cut=4000, aoct=2, gate=0.7, div=8, dly=0.4, rev=0.4, cho=0.25)))

genre('Pop',
      ('Pop Lead', 'lead', dict(w1=SAW, v1=3, d1=0.2, w2=PULSE, v2=1, l2=0.3, cut=5500, res=0.15, vib=0.012, dly=0.25, rev=0.3, cho=0.2)),
      ('Pop Pluck', 'pluck', dict(w1=SAW, v1=3, d1=0.2, cut=4000, res=0.2, fe=0.45, ed=0.15, d=0.3, r=0.3, cho=0.25, dly=0.3, rev=0.3, st=1.0)),
      ('Shiny Pad', 'pad', dict(w1=SUPER, v1=6, d1=0.35, cut=5000, a=0.5, r=1.2, cho=0.35, rev=0.4, st=1.0)),
      ('Pop Bass', 'bass', dict(w1=SAW, v1=1, cut=900, res=0.2, fe=0.3, ed=0.15, es=0.5, d=0.25, s=0.6, r=0.1, comp=1)),
      ('Bright Keys', 'keys', dict(w1=SAW, v1=2, d1=0.1, w2=TRI, v2=1, o2=1, l2=0.3, cut=5500, fe=0.2, d=0.7, s=0.3, r=0.3, cho=0.3, rev=0.25)),
      ('Synth Strings', 'pad', dict(w1=SAW, v1=6, d1=0.4, w2=PULSE, v2=2, l2=0.3, cut=3500, a=0.35, r=0.8, cho=0.45, rev=0.35, st=1.0)),
      ('Hook Arp', 'arp', dict(w1=PWM, lt=T_W1, lw=1, lr=0.5, la=0.4, v1=2, d1=0.2, cut=5000, aoct=2, gate=0.5, dly=0.35, rev=0.3)),
      ('Lift FX', 'fx', dict(w1=NOISE, v1=1, ft=LAD, cut=500, res=0.3, fe=0.8, ea=2.0, ed=1.0, es=1.0, a=2.0, s=1.0, rev=0.4)))

genre('Funk',
      ('Funk Bass', 'bass', dict(w1=PULSE, ft=LAD, cut=500, res=0.4, fe=0.55, ed=0.1, es=0.2, d=0.14, s=0.3, r=0.06, drv=0.2, comp=1)),
      ('Slap Bass', 'bass', dict(w1=SAW, ft=LAD, cut=350, res=0.35, fe=0.7, ed=0.07, es=0.1, d=0.12, s=0.2, r=0.05, drv=0.25, comp=1, treb=2)),
      ('Clav', 'keys', dict(w1=PULSE, cut=2400, res=0.3, fe=0.4, ed=0.1, es=0.1, d=0.2, s=0.15, r=0.1, treb=2)),
      ('Wah Guitar Synth', 'pluck', dict(w1=SAW, v1=2, d1=0.1, ft=LADBP, cut=900, res=0.55, lt=T_CUT, lw=0, lr=3.0, la=0.5, d=0.25, r=0.15, dist=0.2)),
      ('Horn Section', 'pad', dict(w1=SAW, v1=4, d1=0.15, ft=LAD, cut=1400, res=0.15, fe=0.5, ea=0.05, ed=0.2, es=0.7, a=0.04, d=0.25, s=0.7, r=0.15, cho=0.2, rev=0.2)),
      ('Talk Box Lead', 'lead', dict(w1=SAW, v1=2, d1=0.12, ft=LADBP, cut=1100, res=0.6, lt=T_CUT, lw=1, lr=2.0, la=0.45, mono=1, porta=0.04, rev=0.2)),
      ('Synth Funk Lead', 'lead', dict(w1=PWM, lt=T_W1, lw=1, lr=0.6, la=0.4, v1=2, d1=0.15, ft=LAD, cut=2800, res=0.3, porta=0.08, vib=0.012, dly=0.25)),
      ('Organ Groove', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.6, cut=6000, d=0.1, s=1.0, r=0.08, lt=T_AMP, lw=0, lr=6.5, la=0.05, cho=0.35, dist=0.15)))

genre('Disco',
      ('Disco Strings', 'pad', dict(w1=SAW, v1=5, d1=0.3, cut=3500, a=0.2, d=0.5, s=0.9, r=0.5, cho=0.5, rev=0.3, st=1.0, vib=0.008)),
      ('Octave Bass', 'bass', dict(w1=SAW, w2=SAW, v2=1, o2=1, l2=0.6, cut=900, res=0.15, d=0.12, s=0.1, r=0.08, comp=1)),
      ('Disco Stab', 'pluck', dict(w1=SAW, v1=3, d1=0.2, w2=PULSE, v2=1, l2=0.3, cut=3000, fe=0.4, ed=0.14, d=0.2, r=0.15, cho=0.3, rev=0.2)),
      ('Electric Piano', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=2, l2=0.25, cut=5000, d=1.0, s=0.2, r=0.5, cho=0.4, phs=0.3)),
      ('Clavinet', 'keys', dict(w1=PULSE, cut=2600, res=0.25, fe=0.35, ed=0.1, d=0.18, s=0.1, r=0.1, treb=2, phs=0.2)),
      ('Synth Lead', 'lead', dict(w1=SAW, v1=2, d1=0.15, cut=4500, res=0.2, vib=0.015, porta=0.04, dly=0.3, rev=0.3, cho=0.25)),
      ('Mirror Ball Arp', 'arp', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=2, l2=0.3, cut=6000, div=11, aoct=2, gate=0.5, dly=0.3, rev=0.35, cho=0.3)),
      ('Horn Hit', 'pluck', dict(w1=SAW, v1=4, d1=0.15, ft=LAD, cut=1500, fe=0.5, ea=0.04, ed=0.2, d=0.25, r=0.2, rev=0.25)))

genre('Jazz',
      ('Jazz Organ', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.55, cut=6500, d=0.1, s=1.0, r=0.1, lt=T_AMP, lw=0, lr=6.0, la=0.06, cho=0.4, dist=0.1)),
      ('Upright Bass', 'bass', dict(w1=TRI, w2=SINE, v2=1, l2=0.3, cut=600, fe=0.3, ed=0.1, es=0.2, d=0.6, s=0.1, r=0.2, comp=1)),
      ('Vibes', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=2, l2=0.25, cut=6500, d=1.5, s=0.0, r=1.0, lt=T_AMP, lw=0, lr=5.0, la=0.25, rev=0.35)),
      ('Mellow Rhodes', 'keys', dict(w1=SINE, w2=TRI, v2=1, o2=2, l2=0.3, cut=4200, fe=0.1, d=1.6, s=0.25, r=0.7, cho=0.3, phs=0.2, rev=0.3)),
      ('Muted Trumpet', 'lead', dict(w1=SAW, v1=1, ft=LAD, cut=1700, res=0.4, fe=0.4, ea=0.04, ed=0.2, es=0.7, a=0.04, s=0.8, r=0.2, vib=0.012, rev=0.3)),
      ('Sax Lead', 'lead', dict(w1=SAW, v1=2, d1=0.06, w2=PULSE, v2=1, l2=0.25, ft=LAD, cut=1800, res=0.25, a=0.05, s=0.85, r=0.2, vib=0.02, rev=0.3)),
      ('Comp Pad', 'pad', dict(w1=TRI, v1=3, d1=0.12, w2=SINE, v2=1, l2=0.4, cut=2500, a=0.6, r=1.4, cho=0.3, rev=0.4)),
      ('Walking Pluck', 'pluck', dict(w1=TRI, v1=1, w2=SINE, v2=1, o2=1, l2=0.3, cut=2400, fe=0.3, d=0.5, r=0.3, rev=0.25)))

genre('World',
      ('Sitar Pluck', 'pluck', dict(w1=SAW, v1=2, d1=0.08, w2=PULSE, v2=1, s2=7, l2=0.3, ft=1, cut=2600, res=0.55, fe=0.4, ed=0.5, d=1.0, r=0.7, rev=0.3)),
      ('Oud', 'pluck', dict(w1=TRI, v1=2, d1=0.05, w2=SAW, v2=1, l2=0.3, cut=2200, res=0.25, fe=0.35, ed=0.3, d=0.9, r=0.5, rev=0.3, dly=0.2)),
      ('Kalimba', 'pluck', dict(w1=TRI, w2=SINE, v2=1, o2=2, l2=0.35, cut=4500, d=0.6, r=0.5, dly=0.3, rev=0.3)),
      ('Pan Flute', 'lead', dict(w1=SINE, w2=NOISE, v2=1, l2=0.05, ft=LAD, cut=3000, a=0.08, s=0.8, r=0.3, vib=0.025, rev=0.45)),
      ('Duduk', 'lead', dict(w1=SAW, v1=2, d1=0.05, ft=1, cut=1500, res=0.5, a=0.15, s=0.85, r=0.4, vib=0.025, rev=0.4, porta=0.05)),
      ('Tabla Tone', 'pluck', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.3, cut=1800, lt=T_PITCH, lw=3, lr=3.0, la=0.1, a=0.001, d=0.25, s=0.0, r=0.15, rev=0.2)),
      ('Drone', 'pad', dict(w1=SAW, v1=3, d1=0.15, w2=PULSE, v2=1, s2=7, l2=0.3, ft=LAD, cut=1000, res=0.3, a=2.0, r=3.0, rev=0.5, vib=0.006)),
      ('Koto', 'pluck', dict(w1=PULSE, v1=1, w2=TRI, v2=1, o2=1, l2=0.3, cut=3200, res=0.2, fe=0.4, ed=0.2, d=0.8, r=0.6, rev=0.3)))

genre('Folk',
      ('Acoustic Pluck', 'pluck', dict(w1=SAW, v1=1, w2=TRI, v2=1, o2=1, l2=0.3, cut=3200, res=0.15, fe=0.5, ed=0.2, d=0.6, r=0.4, rev=0.25)),
      ('Banjo', 'pluck', dict(w1=PULSE, v1=1, cut=3000, res=0.3, fe=0.5, ed=0.1, d=0.3, r=0.2, rev=0.15, treb=3)),
      ('Accordion', 'pad', dict(w1=PULSE, v1=3, d1=0.2, w2=SAW, v2=2, l2=0.4, cut=3500, a=0.06, d=0.2, s=0.9, r=0.15, lt=T_AMP, lw=0, lr=5.5, la=0.06, rev=0.2)),
      ('Fiddle', 'lead', dict(w1=SAW, v1=2, d1=0.08, ft=LAD, cut=3500, res=0.2, a=0.08, s=0.9, r=0.2, vib=0.02, porta=0.05, rev=0.3)),
      ('Whistle', 'lead', dict(w1=SINE, w2=NOISE, v2=1, l2=0.03, cut=5000, a=0.05, s=0.9, r=0.25, vib=0.02, rev=0.4)),
      ('Folk Bass', 'bass', dict(w1=TRI, w2=SINE, v2=1, l2=0.3, cut=550, d=0.4, s=0.3, r=0.15, fe=0.2, ed=0.1)),
      ('Harmonium', 'keys', dict(w1=SAW, v1=2, d1=0.15, w2=SAW, v2=1, o2=1, l2=0.3, cut=2800, a=0.05, d=0.1, s=1.0, r=0.1, rev=0.2, cho=0.2)),
      ('Pad Strings', 'pad', dict(w1=SAW, v1=4, d1=0.2, cut=2800, a=0.5, r=1.0, cho=0.3, rev=0.4, vib=0.008)))

genre('Baroque',
      ('Harpsichord', 'pluck', dict(w1=PULSE, v1=2, d1=0.05, w2=SAW, v2=1, o2=1, l2=0.3, cut=4500, res=0.15, fe=0.3, ed=0.12, d=0.6, r=0.3, rev=0.3, treb=3)),
      ('Church Organ', 'keys', dict(w1=SINE, v1=1, w2=PULSE, v2=1, o2=1, l2=0.4, cut=5000, a=0.02, d=0.1, s=1.0, r=0.15, rev=0.6, rsz=0.9)),
      ('Pipe Organ Full', 'pad', dict(w1=PULSE, v1=2, d1=0.1, w2=SAW, v2=2, o2=-1, l2=0.5, cut=4000, a=0.03, d=0.1, s=1.0, r=0.2, rev=0.7, rsz=0.95, st=1.0)),
      ('Recorder', 'lead', dict(w1=SINE, w2=TRI, v2=1, o2=1, l2=0.2, cut=4500, a=0.05, s=0.85, r=0.2, vib=0.01, rev=0.4)),
      ('Lute', 'pluck', dict(w1=TRI, v1=2, d1=0.04, w2=SAW, v2=1, l2=0.2, cut=2800, fe=0.35, ed=0.2, d=0.8, r=0.5, rev=0.3)),
      ('Chamber Strings', 'pad', dict(w1=SAW, v1=3, d1=0.15, cut=2800, a=0.25, d=0.5, s=0.9, r=0.5, vib=0.01, cho=0.25, rev=0.4)),
      ('Celeste', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=2, l2=0.3, cut=7000, d=1.2, s=0.0, r=1.0, rev=0.5)),
      ('Baroque Bass', 'bass', dict(w1=TRI, o1=-1, w2=PULSE, v2=1, o2=-1, l2=0.3, cut=900, d=0.3, s=0.6, r=0.2, rev=0.2)))

genre('Cinematic',
      ('Epic Brass', 'pad', dict(w1=SAW, v1=5, d1=0.2, w2=SAW, v2=3, o2=-1, l2=0.5, ft=LAD, cut=1200, res=0.15, fe=0.55, ea=0.12, ed=0.4, es=0.7, a=0.12, d=0.4, s=0.85, r=0.6, rev=0.5, rsz=0.9, bass=3)),
      ('Low Strings', 'pad', dict(w1=SAW, v1=4, d1=0.2, o1=-1, cut=1600, a=0.5, d=0.6, s=0.9, r=1.0, vib=0.008, rev=0.55, rsz=0.9, cho=0.3, st=1.0)),
      ('Hybrid Hit', 'fx', dict(w1=NOISE, v1=1, w2=SINE, v2=1, o2=-2, l2=0.7, cut=2500, a=0.001, d=2.0, s=0.0, r=2.0, rev=0.7, rsz=1.0, comp=1, bass=6, dist=0.2)),
      ('Tension Drone', 'pad', dict(w1=SAW, v1=3, d1=0.4, w2=PULSE, v2=2, o2=-1, l2=0.4, ft=LAD, cut=600, res=0.3, lt=T_CUT, lw=8, lr=0.1, la=0.4, a=3.0, r=4.0, rev=0.6, st=1.0)),
      ('Pulse Ostinato', 'arp', dict(w1=PWM, lt=T_W1, lw=1, lr=0.3, la=0.4, v1=2, ft=LAD, cut=2200, res=0.3, fe=0.3, ed=0.15, es=0.2, div=11, aoct=1, gate=0.5, dly=0.3, rev=0.5)),
      ('Heroic Horn', 'lead', dict(w1=SAW, v1=3, d1=0.12, o1=0, ft=LAD, cut=1400, fe=0.5, ea=0.1, ed=0.3, es=0.75, a=0.12, s=0.85, r=0.5, rev=0.5, vib=0.01)),
      ('Choir Pad', 'pad', dict(w1=TRI, v1=4, d1=0.2, w2=SAW, v2=2, l2=0.3, ft=1, cut=1000, res=0.5, a=0.5, r=1.5, vib=0.012, rev=0.65, rsz=0.95, cho=0.3, st=1.0)),
      ('Braam', 'fx', dict(w1=SAW, v1=4, d1=0.5, o1=-1, w2=PULSE, v2=2, o2=-1, l2=0.5, ft=LAD, cut=500, res=0.35, fe=0.5, ea=0.05, ed=1.0, es=0.5, a=0.03, d=2.0, s=0.4, r=1.5, dist=0.4, dty=1, rev=0.6, comp=1)))

genre('Chiptune',
      ('Pulse Lead', 'lead', dict(w1=PULSE, v1=1, cut=12000, res=0.0, a=0.001, d=0.2, s=0.8, r=0.05, rev=0.0, dly=0.0, vib=0.012)),
      ('Triangle Bass', 'bass', dict(w1=TRI, cut=12000, res=0.0, a=0.001, d=0.2, s=0.9, r=0.03, o1=-1)),
      ('Noise Drum', 'fx', dict(w1=NOISE, v1=1, cut=9000, res=0.0, a=0.001, d=0.1, s=0.0, r=0.05, rev=0.0)),
      ('Arp Chord', 'arp', dict(w1=PULSE, v1=1, cut=12000, res=0.0, a=0.001, d=0.15, s=0.5, r=0.03, div=12, aoct=1, gate=0.7, dly=0.0, rev=0.0)),
      ('PWM Lead', 'lead', dict(w1=PWM, lt=T_W1, lw=1, lr=2.0, la=0.7, v1=1, cut=12000, a=0.001, s=0.8, r=0.05, rev=0.0, dly=0.0)),
      ('Bit Pluck', 'pluck', dict(w1=QUART, v1=1, cut=10000, res=0.0, fe=0.0, d=0.2, r=0.05, rev=0.0, dist=0.2, dty=4)),
      ('Square Duet', 'lead', dict(w1=PULSE, v1=1, w2=HALF, v2=1, o2=1, l2=0.4, cut=12000, res=0.0, a=0.001, s=0.8, r=0.05, rev=0.0, dly=0.0)),
      ('Game Zap', 'fx', dict(w1=PULSE, v1=1, cut=12000, lt=T_PITCH, lw=3, lr=8.0, la=0.7, a=0.001, d=0.15, s=0.0, r=0.05, rev=0.0)))

genre('Retro Poly',
      ('Poly Brass', 'pad', dict(w1=SAW, v1=3, d1=0.15, ft=LAD, cut=1300, res=0.15, fe=0.5, ea=0.05, ed=0.3, es=0.6, a=0.04, d=0.4, s=0.8, r=0.3, cho=0.35, rev=0.2)),
      ('Poly Strings', 'pad', dict(w1=SAW, v1=5, d1=0.3, w2=PULSE, v2=2, l2=0.4, cut=2600, a=0.35, r=0.8, cho=0.5, phs=0.3, rev=0.3, st=1.0)),
      ('Prophet Pad', 'pad', dict(w1=SAW, v1=3, d1=0.2, w2=PWM, v2=2, l2=0.5, lt=T_W1, lw=1, lr=0.3, la=0.4, ft=LAD, cut=1800, res=0.2, a=0.6, r=1.4, cho=0.35, rev=0.3)),
      ('Jupiter Lead', 'lead', dict(w1=SAW, v1=2, d1=0.15, w2=SAW, v2=1, s2=7, l2=0.35, ft=LAD, cut=3500, res=0.25, vib=0.012, porta=0.05, cho=0.3, dly=0.25)),
      ('Juno Chords', 'pad', dict(w1=PWM, lt=T_W1, lw=1, lr=0.5, la=0.5, v1=4, d1=0.2, w2=SAWSUB, v2=2, l2=0.3, ft=LAD, cut=2200, a=0.1, r=0.8, cho=0.6, rev=0.25, st=1.0)),
      ('OB Bass', 'bass', dict(w1=SAW, w2=SAW, v2=1, d2=0.1, l2=0.6, ft=LAD, cut=600, res=0.3, fe=0.4, ed=0.2, es=0.3, d=0.3, s=0.5, r=0.1, drv=0.2)),
      ('Mini Lead', 'lead', dict(w1=SAW, v1=3, d1=0.2, w2=PULSE, v2=1, l2=0.5, ft=LAD, cut=2400, res=0.4, mono=1, porta=0.08, fe=0.3, ed=0.3, es=0.7, drv=0.3)),
      ('Poly Pluck', 'pluck', dict(w1=PULSE, v1=3, d1=0.2, ft=LAD, cut=2600, res=0.3, fe=0.5, ed=0.18, d=0.3, r=0.3, cho=0.4, rev=0.3)))

genre('Sci-Fi',
      ('Alien Lead', 'lead', dict(w1=SAW, v1=2, d1=0.3, w2=QUART, v2=1, s2=5, l2=0.4, ft=1, cut=1800, res=0.5, lt=T_PITCH, lw=1, lr=5.0, la=0.06, mono=1, rev=0.5, dly=0.35)),
      ('Warp Drive', 'fx', dict(w1=SAW, v1=4, d1=0.5, ft=LADBP, cut=300, res=0.5, fe=0.8, ea=3.0, ed=1.5, es=1.0, lt=T_PITCH, lw=2, lr=0.3, la=0.4, a=3.0, s=1.0, rev=0.5)),
      ('Computer Beeps', 'arp', dict(w1=PULSE, v1=1, cut=6000, res=0.3, div=12, aoct=3, gate=0.3, pat=5, dly=0.4, rev=0.3, a=0.001, d=0.1, s=0.0, r=0.1)),
      ('Space Pad', 'pad', dict(w1=SINE, v1=3, d1=0.4, w2=TRI, v2=2, o2=1, l2=0.4, cut=5000, a=2.5, r=4.0, phs=0.5, rev=0.7, rsz=1.0, dly=0.4, st=1.0)),
      ('Ray Gun', 'fx', dict(w1=PULSE, v1=1, w2=SAW, v2=1, l2=0.5, cut=7000, res=0.5, lt=T_PITCH, lw=3, lr=5.0, la=0.6, a=0.001, d=0.4, s=0.0, r=0.2, dly=0.3)),
      ('Android Voice', 'lead', dict(w1=PULSE, v1=2, d1=0.3, ft=LADBP, cut=1400, res=0.55, lt=T_CUT, lw=9, lr=6.0, la=0.4, dist=0.3, dty=3, dly=0.25)),
      ('Reactor Hum', 'pad', dict(w1=SAW, v1=3, d1=0.3, o1=-1, ft=LAD, cut=500, res=0.4, lt=T_CUT, lw=0, lr=0.4, la=0.3, a=2.0, r=3.0, rev=0.5)),
      ('Alien Bell', 'keys', dict(w1=SINE, w2=QUART, v2=1, o2=2, s2=5, l2=0.35, cut=9000, res=0.2, d=1.8, s=0.0, r=1.5, rev=0.6, dly=0.4, phs=0.4)))

genre('Horror',
      ('Dread Drone', 'pad', dict(w1=SAW, v1=3, d1=0.5, o1=-1, w2=PULSE, v2=2, o2=-1, l2=0.4, ft=LAD, cut=400, res=0.4, lt=T_CUT, lw=8, lr=0.08, la=0.4, a=4.0, r=5.0, rev=0.7, rsz=1.0, st=1.0)),
      ('Music Box Creepy', 'keys', dict(w1=TRI, w2=SINE, v2=1, o2=2, s2=1, l2=0.3, cut=5500, lt=T_PITCH, lw=8, lr=0.4, la=0.02, d=1.0, s=0.0, r=0.8, rev=0.6, dly=0.4, treb=-2)),
      ('Screech Stab', 'fx', dict(w1=SAW, v1=4, d1=0.7, w2=SAW, v2=2, s2=6, l2=0.5, cut=6000, res=0.4, lt=T_PITCH, lw=3, lr=3.0, la=0.1, a=0.001, d=0.8, s=0.0, r=0.6, dist=0.5, dty=2, rev=0.6)),
      ('Whisper Wind', 'fx', dict(w1=NOISE, v1=1, ft=LADBP, cut=900, res=0.5, lt=T_CUT, lw=8, lr=0.25, la=0.5, a=2.0, r=3.0, rev=0.7)),
      ('Low Pulse', 'bass', dict(w1=SINE, w2=PULSE, v2=1, o2=-1, l2=0.3, cut=300, lt=T_AMP, lw=0, lr=1.0, la=0.5, a=0.5, d=1.0, s=0.9, r=1.5, rev=0.5, bass=4)),
      ('Detuned Choir', 'pad', dict(w1=TRI, v1=4, d1=0.6, w2=SAW, v2=2, d2=0.7, l2=0.3, ft=1, cut=900, res=0.5, lt=T_PITCH, lw=8, lr=0.3, la=0.03, a=1.5, r=3.0, vib=0.02, rev=0.7, st=1.0)),
      ('Heartbeat', 'bass', dict(w1=SINE, cut=300, lt=T_PITCH, lw=3, lr=1.2, la=0.2, a=0.001, d=0.4, s=0.0, r=0.3, rev=0.3, bass=5, comp=1)),
      ('Static Texture', 'fx', dict(w1=NOISE, v1=1, w2=PULSE, v2=1, l2=0.2, ft=LADHP, cut=2000, lt=T_CUT, lw=7, lr=10.0, la=0.5, a=1.0, r=2.0, rev=0.5, dist=0.3)))

genre('Reggae',
      ('Dub Bass', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.2, cut=600, d=0.5, s=0.7, r=0.3, bass=4, comp=1, rev=0.15)),
      ('Skank Organ', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.5, cut=5000, a=0.002, d=0.12, s=0.0, r=0.06, dist=0.15, cho=0.25, rev=0.2)),
      ('Bubble Organ', 'keys', dict(w1=SINE, w2=PULSE, v2=1, o2=1, l2=0.3, cut=5500, d=0.1, s=0.9, r=0.1, lt=T_AMP, lw=0, lr=6.0, la=0.06, rev=0.25)),
      ('Horn Skank', 'pluck', dict(w1=SAW, v1=3, d1=0.12, ft=LAD, cut=1300, fe=0.45, ea=0.03, ed=0.15, d=0.2, r=0.1, rev=0.3)),
      ('Echo Chord', 'pluck', dict(w1=PULSE, v1=2, d1=0.15, cut=2500, fe=0.3, ed=0.15, d=0.2, r=0.15, dly=0.55, dfb=0.65, rev=0.3)),
      ('Melodica', 'lead', dict(w1=SAW, v1=1, w2=PULSE, v2=1, l2=0.3, ft=LAD, cut=2800, a=0.04, s=0.9, r=0.15, vib=0.012, dly=0.35, rev=0.3)),
      ('Dub Siren', 'fx', dict(w1=SINE, cut=8000, lt=T_PITCH, lw=3, lr=1.0, la=0.4, a=0.01, d=1.0, s=0.8, r=1.0, dly=0.55, dfb=0.7, rev=0.4)),
      ('Warm Pad', 'pad', dict(w1=TRI, v1=3, d1=0.15, cut=2000, a=0.6, r=1.3, cho=0.35, rev=0.4, dly=0.25)))

genre('Gospel',
      ('Hammond', 'keys', dict(w1=SINE, w2=SINE, v2=1, o2=1, l2=0.6, cut=7000, d=0.1, s=1.0, r=0.1, lt=T_AMP, lw=0, lr=6.5, la=0.06, cho=0.5, dist=0.2, dty=1, rev=0.25)),
      ('Choir Aah', 'pad', dict(w1=SAW, v1=5, d1=0.2, w2=TRI, v2=2, l2=0.5, ft=1, cut=950, res=0.5, a=0.3, r=1.0, vib=0.012, rev=0.55, cho=0.3, st=1.0)),
      ('Church Piano', 'keys', dict(w1=TRI, v1=2, d1=0.06, w2=SINE, v2=1, o2=1, l2=0.3, cut=3200, fe=0.2, d=1.6, s=0.1, r=0.7, rev=0.4)),
      ('Pipe Swell', 'pad', dict(w1=PULSE, v1=2, d1=0.1, w2=SAW, v2=2, o2=-1, l2=0.5, cut=3800, a=0.4, s=1.0, r=0.4, rev=0.7, rsz=0.9, st=1.0)),
      ('Praise Lead', 'lead', dict(w1=SAW, v1=3, d1=0.15, w2=PULSE, v2=1, l2=0.3, ft=LAD, cut=3800, a=0.02, s=0.9, r=0.3, vib=0.018, rev=0.4, dly=0.3)),
      ('Walking Bass', 'bass', dict(w1=SINE, w2=TRI, v2=1, l2=0.3, cut=700, fe=0.2, ed=0.1, d=0.5, s=0.3, r=0.2, comp=1)),
      ('Tambourine Synth', 'fx', dict(w1=NOISE, v1=1, ft=LADHP, cut=5000, a=0.001, d=0.08, s=0.0, r=0.05, rev=0.2)),
      ('Glory Strings', 'pad', dict(w1=SUPER, v1=5, d1=0.3, cut=3500, a=0.5, r=1.2, cho=0.4, rev=0.5, st=1.0)))
