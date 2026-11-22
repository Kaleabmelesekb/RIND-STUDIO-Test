// SPDX-License-Identifier: GPL-3.0-or-later
// Honeydew's own additions in the ML Studio style: a bitcrusher, a harmonic exciter and a spectral compressor.
use crate::*;
use ml_small_fx::{Card, Control, Mark, Setting};

pub type Bitcrusher = Fx<Crusher>;
pub type Exciter = Fx<Excite>;
pub type SpectralCompressor = Fx<Spectral>;

const fn setting(id: &'static str, name: &'static str, range: (i32, i32), start: i32) -> Setting {
    Setting {
        id,
        name,
        range,
        start,
        magnet: None,
    }
}
const fn output() -> Setting {
    Setting {
        id: "output",
        name: "Output",
        range: (-240, 120),
        start: 0,
        magnet: Some(0),
    }
}
const fn card(n: usize, heading: &'static str) -> Card {
    Card {
        x0: 8. + 124. * n as f32,
        x1: 124. + 124. * n as f32,
        y0: 28.,
        y1: 153.,
        heading,
    }
}
const fn knob(n: usize, id: &'static str, bipolar: bool) -> Control {
    Control::Knob {
        id,
        cx: 66. + 124. * n as f32,
        cy: 85.,
        r: 24.,
        caption: "",
        bipolar,
    }
}
const fn mark(n: usize) -> Mark {
    Mark {
        x: 124. * n as f32,
        y: 172.,
        anchor: 1.,
        stacked: false,
    }
}
fn db(v: i32) -> f64 {
    10f64.powf(v as f64 / 200.)
}
/// A typed value: its leading number times `scale`, kept in the setting's range.
fn parse_with(spec: &Spec, scales: &[f64], k: usize, s: &str) -> Option<i32> {
    let v = s
        .split_whitespace()
        .next()?
        .split(':')
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    if !v.is_finite() {
        return None;
    }
    let p = spec.settings.get(k)?;
    Some(((v * scales.get(k)?).round() as i32).clamp(p.range.0, p.range.1))
}
fn tenths(v: i32) -> String {
    format!("{:+.1} dB", v as f64 / 10.)
}
fn percent(v: i32) -> String {
    format!("{:.0}%", v as f64 / 10.)
}
/// A panel with a few lines of text under the cards.
pub struct Notes(pub &'static [&'static str]);
impl Extension for Notes {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        for (i, line) in self.0.iter().enumerate() {
            text(c, 16., 163. + 16. * i as f32, line);
        }
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}

// ---- Bitcrusher ----

pub static CRUSHER: Spec = Spec {
    name: "Rind Bitcrusher",
    word: "BITCRUSHER",
    about: "Honeydew's bitcrusher in the ML Studio look.",
    settings: &[
        setting("bits", "Bits", (1, 16), 8),
        setting("rate", "Downsample", (1, 64), 1),
        setting("mix", "Mix", (0, 1000), 1000),
        output(),
    ],
    studio_settings: 4,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (512, 190),
    cards: &[
        card(0, "BITS"),
        card(1, "DOWNSAMPLE"),
        card(2, "MIX"),
        card(3, "OUTPUT"),
    ],
    controls: &[
        knob(0, "bits", false),
        knob(1, "rate", false),
        knob(2, "mix", false),
        knob(3, "output", true),
    ],
    display: None,
    mark: mark(4),
    hint: None,
    labels: &[],
    text: crusher_text,
    parse: crusher_parse,
    sound: |_, _| unreachable!(),
    rings: false,
};
fn crusher_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{v} bit"),
        1 => format!("1/{v}"),
        2 => percent(v),
        3 => tenths(v),
        _ => String::new(),
    }
}
fn crusher_parse(k: usize, s: &str) -> Option<i32> {
    parse_with(&CRUSHER, &[1., 1., 10., 10.], k, s)
}
#[derive(Default)]
pub struct Crusher {
    held: [f32; 2],
    count: [u32; 2],
}
impl Unit for Crusher {
    const SPEC: &'static Spec = &CRUSHER;
    const ID: &'static str = "org.honeydew.ml-bitcrusher";
    const CLASS: [u8; 16] = *b"MLBitcrusher0001";
    fn prepare(&mut self, _: f32, p: &FxParams) {
        self.reset(p)
    }
    fn reset(&mut self, _: &FxParams) {
        *self = Self::default();
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let half = (1u32 << (p.v(0).clamp(1, 16) - 1)) as f32;
        let every = p.v(1).clamp(1, 64) as u32;
        let mix = p.v(2) as f32 / 1000.;
        let gain = db(p.v(3)) as f32;
        for (ch, buf) in [l, r].into_iter().enumerate() {
            for s in buf.iter_mut() {
                let dry = if s.is_finite() { *s } else { 0. };
                if self.count[ch] == 0 {
                    self.held[ch] = (dry.clamp(-1., 1.) * half).round() / half;
                }
                self.count[ch] = (self.count[ch] + 1) % every;
                *s = (dry + (self.held[ch] - dry) * mix) * gain;
            }
        }
    }
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[
            "Fewer bits coarsen each sample; Downsample holds every Nth sample.",
        ]))
    }
}

// ---- Exciter ----

pub static EXCITER: Spec = Spec {
    name: "Rind Exciter",
    word: "EXCITER",
    about: "Honeydew's harmonic exciter in the ML Studio look.",
    settings: &[
        setting("freq", "Frequency", (500, 16000), 3000),
        setting("drive", "Drive", (0, 360), 120),
        setting("amount", "Amount", (0, 1000), 300),
        output(),
    ],
    studio_settings: 4,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (512, 190),
    cards: &[
        card(0, "FREQUENCY"),
        card(1, "DRIVE"),
        card(2, "AMOUNT"),
        card(3, "OUTPUT"),
    ],
    controls: &[
        knob(0, "freq", false),
        knob(1, "drive", false),
        knob(2, "amount", false),
        knob(3, "output", true),
    ],
    display: None,
    mark: mark(4),
    hint: None,
    labels: &[],
    text: exciter_text,
    parse: exciter_parse,
    sound: |_, _| unreachable!(),
    rings: false,
};
fn exciter_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{v} Hz"),
        1 => format!("{:.1} dB", v as f64 / 10.),
        2 => percent(v),
        3 => tenths(v),
        _ => String::new(),
    }
}
fn exciter_parse(k: usize, s: &str) -> Option<i32> {
    parse_with(&EXCITER, &[1., 10., 10., 10.], k, s)
}
#[derive(Default, Clone, Copy)]
struct Pole(f32);
impl Pole {
    /// The high-passed sample: the input minus a one-pole low-pass of coefficient `a`.
    fn high(&mut self, x: f32, a: f32) -> f32 {
        self.0 += a * (x - self.0);
        x - self.0
    }
}
#[derive(Default)]
pub struct Excite {
    rate: f32,
    before: [[Pole; 2]; 2],
    after: [[Pole; 2]; 2],
}
impl Unit for Excite {
    const SPEC: &'static Spec = &EXCITER;
    const ID: &'static str = "org.honeydew.ml-exciter";
    const CLASS: [u8; 16] = *b"MLExciter0000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.reset(p);
        self.rate = rate;
    }
    fn reset(&mut self, _: &FxParams) {
        let rate = self.rate;
        *self = Self::default();
        self.rate = rate;
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let rate = self.rate.max(8000.);
        let freq = (p.v(0) as f32).min(rate * 0.45);
        let a = 1. - (-std::f32::consts::TAU * freq / rate).exp();
        let drive = db(p.v(1)) as f32;
        let amount = p.v(2) as f32 / 1000.;
        let gain = db(p.v(3)) as f32;
        let bias = 0.2f32;
        let rest = bias.tanh();
        for (ch, buf) in [l, r].into_iter().enumerate() {
            for s in buf.iter_mut() {
                let dry = if s.is_finite() { *s } else { 0. };
                let first = self.before[ch][0].high(dry, a);
                let high = self.before[ch][1].high(first, a);
                let sat = (high * drive + bias).tanh() - rest;
                let first = self.after[ch][0].high(sat, a);
                let sat = self.after[ch][1].high(first, a);
                *s = (dry + sat * amount) * gain;
            }
        }
    }
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[
            "Saturates only what is above Frequency and adds it back to the dry signal.",
        ]))
    }
}

// ---- Spectral compressor ----

pub static SPECTRAL: Spec = Spec {
    name: "Rind Spectral Compressor",
    word: "SPECTRAL COMPRESSOR",
    about: "Honeydew's spectral compressor in the ML Studio look.",
    settings: &[
        setting("threshold", "Threshold", (-600, 0), -300),
        setting("ratio", "Ratio", (10, 200), 40),
        setting("attack", "Attack", (1, 500), 30),
        setting("release", "Release", (10, 2000), 300),
        setting("mix", "Mix", (0, 1000), 1000),
        output(),
    ],
    studio_settings: 6,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 190),
    cards: &[
        card(0, "THRESHOLD"),
        card(1, "RATIO"),
        card(2, "ATTACK"),
        card(3, "RELEASE"),
        card(4, "MIX"),
        card(5, "OUTPUT"),
    ],
    controls: &[
        knob(0, "threshold", false),
        knob(1, "ratio", false),
        knob(2, "attack", false),
        knob(3, "release", false),
        knob(4, "mix", false),
        knob(5, "output", true),
    ],
    display: None,
    mark: mark(6),
    hint: None,
    labels: &[],
    text: spectral_text,
    parse: spectral_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn spectral_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:.1} dB", v as f64 / 10.),
        1 => format!("{:.1}:1", v as f64 / 10.),
        2 | 3 => format!("{v} ms"),
        4 => percent(v),
        5 => tenths(v),
        _ => String::new(),
    }
}
fn spectral_parse(k: usize, s: &str) -> Option<i32> {
    parse_with(&SPECTRAL, &[10., 10., 1., 1., 10., 10.], k, s)
}

const N: usize = 2048;
const HOP: usize = 512;
const BINS: usize = N / 2 + 1;
/// A full-scale sine's bin magnitude under the square-root Hann window: the level the threshold is measured from.
const REF: f32 = N as f32 * 0.3183;

struct Fft {
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<u16>,
}
impl Fft {
    fn new() -> Self {
        let bits = N.trailing_zeros();
        Self {
            cos: (0..N / 2)
                .map(|k| (-std::f64::consts::TAU * k as f64 / N as f64).cos() as f32)
                .collect(),
            sin: (0..N / 2)
                .map(|k| (-std::f64::consts::TAU * k as f64 / N as f64).sin() as f32)
                .collect(),
            rev: (0..N)
                .map(|i| (i.reverse_bits() >> (usize::BITS - bits)) as u16)
                .collect(),
        }
    }
    /// An in-place transform (unscaled; the inverse is conjugated).
    fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        for i in 0..N {
            let j = self.rev[i] as usize;
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= N {
            let half = len / 2;
            let step = N / len;
            for start in (0..N).step_by(len) {
                for k in 0..half {
                    let c = self.cos[k * step];
                    let s = if inverse {
                        -self.sin[k * step]
                    } else {
                        self.sin[k * step]
                    };
                    let (a, b) = (start + k, start + k + half);
                    let tr = re[b] * c - im[b] * s;
                    let ti = re[b] * s + im[b] * c;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len *= 2;
        }
    }
}
struct Channel {
    input: Vec<f32>,
    acc: Vec<f32>,
    env: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
}
impl Channel {
    fn new() -> Self {
        Self {
            input: vec![0.; N],
            acc: vec![0.; N],
            env: vec![-120.; BINS],
            re: vec![0.; N],
            im: vec![0.; N],
        }
    }
}
pub struct Spectral {
    rate: f32,
    fft: Fft,
    window: Vec<f32>,
    ch: [Channel; 2],
    pos: usize,
}
impl Default for Spectral {
    fn default() -> Self {
        Self {
            rate: 48000.,
            fft: Fft::new(),
            window: (0..N)
                .map(|n| {
                    let hann = 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / N as f64).cos();
                    hann.sqrt() as f32
                })
                .collect(),
            ch: [Channel::new(), Channel::new()],
            pos: 0,
        }
    }
}
impl Spectral {
    /// One frame of channel `c`, ending at `pos`: the oldest `N` samples, compressed bin by bin.
    fn frame(&mut self, c: usize, threshold: f32, slope: f32, attack: f32, release: f32) {
        let ch = &mut self.ch[c];
        for n in 0..N {
            ch.re[n] = ch.input[(self.pos + n) % N] * self.window[n];
            ch.im[n] = 0.;
        }
        self.fft.run(&mut ch.re, &mut ch.im, false);
        for k in 0..BINS {
            let mag = ch.re[k].hypot(ch.im[k]) / REF;
            let level = 20. * mag.max(1e-9).log10();
            let env = &mut ch.env[k];
            let coef = if level > *env { attack } else { release };
            *env = level + coef * (*env - level);
            let over = *env - threshold;
            let gain = if over > 0. {
                10f32.powf(-over * slope / 20.)
            } else {
                1.
            };
            ch.re[k] *= gain;
            ch.im[k] *= gain;
            if k > 0 && k < N / 2 {
                ch.re[N - k] = ch.re[k];
                ch.im[N - k] = -ch.im[k];
            }
        }
        self.fft.run(&mut ch.re, &mut ch.im, true);
        let scale = 0.5 / N as f32;
        for n in 0..N {
            ch.acc[(self.pos + n) % N] += ch.re[n] * self.window[n] * scale;
        }
    }
}
impl Unit for Spectral {
    const SPEC: &'static Spec = &SPECTRAL;
    const ID: &'static str = "org.honeydew.ml-spectral-compressor";
    const CLASS: [u8; 16] = *b"MLSpectralComp01";
    const LATENCY: u32 = N as u32;
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.reset(p);
        self.rate = rate;
    }
    fn reset(&mut self, _: &FxParams) {
        self.ch = [Channel::new(), Channel::new()];
        self.pos = 0;
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let threshold = p.v(0) as f32 / 10.;
        let slope = 1. - 10. / p.v(1).max(10) as f32;
        let frame_s = HOP as f32 / self.rate.max(8000.);
        let coef = |ms: i32| (-frame_s / (ms.max(1) as f32 / 1000.)).exp();
        let (attack, release) = (coef(p.v(2)), coef(p.v(3)));
        let mix = p.v(4) as f32 / 1000.;
        let gain = db(p.v(5)) as f32;
        for i in 0..l.len() {
            for (c, buf) in [&mut *l, &mut *r].into_iter().enumerate() {
                let x = if buf[i].is_finite() { buf[i] } else { 0. };
                let ch = &mut self.ch[c];
                let dry = ch.input[self.pos];
                ch.input[self.pos] = x;
                let wet = ch.acc[self.pos];
                ch.acc[self.pos] = 0.;
                buf[i] = (dry + (wet - dry) * mix) * gain;
            }
            self.pos = (self.pos + 1) % N;
            if self.pos % HOP == 0 {
                for c in 0..2 {
                    self.frame(c, threshold, slope, attack, release);
                }
            }
        }
    }
    fn panel(_: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Notes(&[
            "Compresses every frequency on its own, so loud peaks are tamed where they are.",
            "Reports 2048 samples of latency, which the host compensates.",
        ]))
    }
}
