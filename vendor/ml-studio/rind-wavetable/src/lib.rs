// SPDX-License-Identifier: GPL-3.0-or-later
// Rind Wavetable: the sound engine of J-SYN (https://github.com/jakehukkanen/J-SYN, GPL-3.0-or-later, `vendor/j-syn-dsp`)
// behind the generated ML Studio editor. The editor and these settings are Honeydew's; the sound is J-SYN's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use j_syn_dsp::{
    AdsrParams, FactoryWavetable, FilterMode, ModDestination, ModMatrixSource, ModSlot, SynthEngine,
    SynthParams, WavetableBank,
};
use ml_mix_core::{extra::Notes, instrument::Instrument, instrument::Synth};
use ml_small_fx::{Card, Control, Mark, Setting, Spec, editor::Extension, params::FxParams};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::sync::Arc;

pub type Effect = Instrument<Wavetable>;

const fn s(id: &'static str, name: &'static str, range: (i32, i32), start: i32) -> Setting {
    Setting {
        id,
        name,
        range,
        start,
        magnet: None,
    }
}
const fn card(row: usize, col: usize, heading: &'static str) -> Card {
    Card {
        x0: 8. + 124. * col as f32,
        x1: 124. + 124. * col as f32,
        y0: 28. + 137. * row as f32,
        y1: 153. + 137. * row as f32,
        heading,
    }
}
const fn knob(row: usize, col: usize, id: &'static str, bipolar: bool) -> Control {
    Control::Knob {
        id,
        cx: 66. + 124. * col as f32,
        cy: 85. + 137. * row as f32,
        r: 24.,
        caption: "",
        bipolar,
    }
}

pub static SPEC: Spec = Spec {
    name: "Rind Wavetable",
    word: "WAVETABLE",
    about: "A wavetable synth: the J-SYN engine in the ML Studio look.",
    settings: &[
        s("wave_a", "Wave A", (1, 4), 1),
        s("pos_a", "Position A", (0, 1000), 0),
        s("wave_b", "Wave B", (1, 4), 2),
        s("pos_b", "Position B", (0, 1000), 0),
        s("level_b", "Level B", (0, 1000), 0),
        s("tune_b", "Tune B", (-24, 24), 0),
        s("unison", "Unison", (1, 7), 1),
        s("detune", "Detune", (0, 1000), 200),
        s("cutoff", "Cutoff", (0, 1000), 1000),
        s("res", "Resonance", (0, 1000), 100),
        s("envamt", "Filter Env", (-1000, 1000), 0),
        s("mode", "Filter", (1, 3), 1),
        s("attack", "Attack", (1, 5000), 5),
        s("decay", "Decay", (1, 5000), 300),
        s("sustain", "Sustain", (0, 1000), 800),
        s("release", "Release", (1, 8000), 250),
        s("glide", "Glide", (0, 2000), 0),
        Setting {
            id: "output",
            name: "Output",
            range: (-240, 120),
            start: 0,
            magnet: Some(0),
        },
    ],
    studio_settings: 18,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 470),
    cards: &[
        card(0, 0, "WAVE A"),
        card(0, 1, "POSITION A"),
        card(0, 2, "WAVE B"),
        card(0, 3, "POSITION B"),
        card(0, 4, "LEVEL B"),
        card(0, 5, "TUNE B"),
        card(1, 0, "UNISON"),
        card(1, 1, "DETUNE"),
        card(1, 2, "CUTOFF"),
        card(1, 3, "RESONANCE"),
        card(1, 4, "FILTER ENV"),
        card(1, 5, "FILTER"),
        card(2, 0, "ATTACK"),
        card(2, 1, "DECAY"),
        card(2, 2, "SUSTAIN"),
        card(2, 3, "RELEASE"),
        card(2, 4, "GLIDE"),
        card(2, 5, "OUTPUT"),
    ],
    controls: &[
        knob(0, 0, "wave_a", false),
        knob(0, 1, "pos_a", false),
        knob(0, 2, "wave_b", false),
        knob(0, 3, "pos_b", false),
        knob(0, 4, "level_b", false),
        knob(0, 5, "tune_b", true),
        knob(1, 0, "unison", false),
        knob(1, 1, "detune", false),
        knob(1, 2, "cutoff", false),
        knob(1, 3, "res", false),
        knob(1, 4, "envamt", true),
        knob(1, 5, "mode", false),
        knob(2, 0, "attack", false),
        knob(2, 1, "decay", false),
        knob(2, 2, "sustain", false),
        knob(2, 3, "release", false),
        knob(2, 4, "glide", false),
        knob(2, 5, "output", true),
    ],
    display: None,
    mark: Mark {
        x: 744.,
        y: 452.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text,
    parse,
    sound: |_, _| unreachable!(),
    rings: true,
};

fn cutoff_hz(v: i32) -> f32 {
    20. * 1000f32.powf(v as f32 / 1000.)
}
fn text(k: usize, v: i32) -> String {
    match k {
        0 | 2 => FactoryWavetable::from_index((v - 1).max(0) as usize).name().to_string(),
        1 | 3 | 4 | 7 | 9 | 14 => format!("{:.0}%", v as f64 / 10.),
        5 => format!("{v:+} st"),
        6 => format!("{v}"),
        8 => format!("{:.0} Hz", cutoff_hz(v)),
        10 => format!("{:+.0}%", v as f64 / 10.),
        11 => ["Low-pass", "Band-pass", "High-pass"][(v - 1).clamp(0, 2) as usize].to_string(),
        12 | 13 | 15 | 16 => format!("{v} ms"),
        17 => format!("{:+.1} dB", v as f64 / 10.),
        _ => String::new(),
    }
}
fn parse(k: usize, t: &str) -> Option<i32> {
    let word = t.split_whitespace().next()?;
    let (num, khz) = match word.strip_suffix(['k', 'K']) {
        Some(n) => (n, true),
        None => (word.trim_end_matches('%'), false),
    };
    let v = num.parse::<f64>().ok().filter(|v| v.is_finite())?;
    let p = SPEC.settings.get(k)?;
    let raw = match k {
        1 | 3 | 4 | 7 | 9 | 10 | 14 | 17 => v * 10.,
        8 => {
            let hz = if khz { v * 1000. } else { v }.clamp(20., 20000.);
            (hz / 20.).log10() / 3. * 1000.
        }
        _ => v,
    };
    Some((raw.round() as i32).clamp(p.range.0, p.range.1))
}

pub struct Wavetable {
    engine: SynthEngine,
    bank: WavetableBank,
    params: SynthParams,
    tables: [i32; 2],
    rate: f32,
}
impl Default for Wavetable {
    fn default() -> Self {
        Self {
            engine: SynthEngine::new(48000.),
            bank: WavetableBank::new(),
            params: SynthParams::default(),
            tables: [0, 0],
            rate: 48000.,
        }
    }
}
impl Wavetable {
    /// The settings into the engine's parameters (cheap: plain field writes).
    fn apply(&mut self, p: &FxParams) {
        let f = |k: usize| p.v(k) as f32;
        for (osc, (wave, k)) in [(p.v(0), 0usize), (p.v(2), 1usize)].into_iter().enumerate() {
            let _ = k;
            if self.tables[osc] != wave {
                self.tables[osc] = wave;
                self.bank.select_factory(
                    osc,
                    FactoryWavetable::from_index((wave - 1).max(0) as usize),
                );
            }
        }
        let sp = &mut self.params;
        sp.oscs[0].wt_pos = f(1) / 1000.;
        sp.oscs[0].level = 0.75;
        sp.oscs[1].enabled = p.v(4) > 0;
        sp.oscs[1].wt_pos = f(3) / 1000.;
        sp.oscs[1].level = f(4) / 1000.;
        sp.oscs[1].pitch_semitones = f(5);
        for osc in 0..2 {
            sp.oscs[osc].unison = p.v(6).clamp(1, 7) as usize;
            sp.oscs[osc].detune = f(7) / 1000.;
        }
        sp.filters[0].enabled = true;
        sp.filters[0].mode = match p.v(11) {
            2 => FilterMode::Bp,
            3 => FilterMode::Hp,
            _ => FilterMode::Lp,
        };
        sp.filters[0].cutoff_hz = cutoff_hz(p.v(8));
        sp.filters[0].resonance = f(9) / 1000.;
        let env = AdsrParams {
            attack_s: f(12) / 1000.,
            decay_s: f(13) / 1000.,
            sustain: f(14) / 1000.,
            release_s: f(15) / 1000.,
        };
        sp.amp_attack_s = env.attack_s;
        sp.amp_decay_s = env.decay_s;
        sp.amp_sustain = env.sustain;
        sp.amp_release_s = env.release_s;
        sp.mod_envs[0] = env;
        sp.mod_matrix.slots[0] = ModSlot {
            source: ModMatrixSource::Env1,
            destination: ModDestination::Filter1Cutoff,
            depth: f(10) / 1000.,
            bipolar: false,
            enabled: p.v(10) != 0,
            bypassed: false,
            aux_source: ModMatrixSource::None,
            curve: 0.,
        };
        sp.glide_ms = f(16);
        sp.master_gain = 0.707_106_77 * 10f32.powf(f(17) / 200.);
    }
}
impl Synth for Wavetable {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-wavetable";
    const CLASS: [u8; 16] = *b"RindWavetable001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate;
        self.engine = SynthEngine::new(rate);
        self.tables = [0, 0];
        self.apply(p);
    }
    fn reset(&mut self, _: &FxParams) {
        self.engine.reset();
    }
    fn note_on(&mut self, channel: u8, note: u8, velocity: f32, voice_id: Option<i32>) {
        self.engine
            .note_on(channel, note, velocity, voice_id, &self.params);
    }
    fn note_off(&mut self, channel: u8, note: u8, voice_id: Option<i32>) {
        self.engine
            .note_off(channel, note, voice_id, &self.params);
    }
    fn pitch_bend(&mut self, value: f32) {
        self.engine.set_pitch_bend(value);
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        self.apply(p);
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            let (a, b) = self.engine.process_sample(&self.params, &self.bank);
            *l = if a.is_finite() { a } else { 0. };
            *r = if b.is_finite() { b } else { 0. };
        }
    }
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[
            "The sound engine is J-SYN's (GPL-3.0): four wavetables, unison and a multimode filter.",
        ]))
    }
}

#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);
