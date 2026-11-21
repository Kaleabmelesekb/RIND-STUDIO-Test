use crate::*;
use std::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
pub type Tuner = Fx<Detector>;
const QUEUE: usize = 8192;
const FRAME: usize = 3072;
const WINDOW: usize = 2048;
struct Readings {
    data: Box<[AtomicU32]>,
    write: AtomicUsize,
    read: AtomicUsize,
    generation: AtomicU64,
    overrun: AtomicU64,
    rate: AtomicU32,
    gate: AtomicU32,
    hz: AtomicU32,
    confidence: AtomicU32,
    updated: AtomicU64,
    published: AtomicU64,
    clock: Instant,
    stop: AtomicBool,
}
impl Readings {
    fn new() -> Self {
        Self {
            data: (0..QUEUE).map(|_| AtomicU32::new(0)).collect(),
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
            generation: AtomicU64::new(1),
            overrun: AtomicU64::new(0),
            rate: AtomicU32::new(24000f32.to_bits()),
            gate: AtomicU32::new(0.001f32.to_bits()),
            hz: AtomicU32::new(f32::NAN.to_bits()),
            confidence: AtomicU32::new(0),
            updated: AtomicU64::new(0),
            published: AtomicU64::new(0),
            clock: Instant::now(),
            stop: AtomicBool::new(false),
        }
    }
    fn invalidate(&self) {
        self.hz.store(f32::NAN.to_bits(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    fn push(&self, x: f32) {
        let w = self.write.load(Ordering::Relaxed);
        let next = (w + 1) % QUEUE;
        if next == self.read.load(Ordering::Acquire) {
            self.overrun.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.data[w].store(x.to_bits(), Ordering::Relaxed);
        self.write.store(next, Ordering::Release);
    }
    fn pop(&self) -> Option<f32> {
        let r = self.read.load(Ordering::Relaxed);
        if r == self.write.load(Ordering::Acquire) {
            return None;
        }
        let x = f32::from_bits(self.data[r].load(Ordering::Relaxed));
        self.read.store((r + 1) % QUEUE, Ordering::Release);
        Some(x)
    }
    fn reading(&self) -> f32 {
        if self.published.load(Ordering::Acquire) != self.generation.load(Ordering::Acquire)
            || self.clock.elapsed().as_millis() as u64 > self.updated.load(Ordering::Acquire) + 500
        {
            f32::NAN
        } else {
            f32::from_bits(self.hz.load(Ordering::Relaxed))
        }
    }
}
/// Native normalized-difference (YIN) estimator. All arrays belong to the worker.
pub(crate) fn estimate(z: &[f32; FRAME], at: usize, rate: f64, gate: f64) -> (f32, f32) {
    let samples: [f64; FRAME] = std::array::from_fn(|i| z[(at + FRAME - 1 - i) % FRAME] as f64);
    let rms = (samples[..WINDOW].iter().map(|x| x * x).sum::<f64>() / WINDOW as f64).sqrt();
    if rms < gate || !rms.is_finite() {
        return (f32::NAN, 0.);
    }
    let max = (rate / 27.5).ceil().min(1022.) as usize;
    let min = (rate / 2000.).floor().max(2.) as usize;
    let mut raw = [0.; 1025];
    let mut norm = [1.; 1025];
    let mut sum = 0.;
    for lag in 1..=max + 1 {
        let d = (0..WINDOW)
            .map(|i| {
                let x = samples[i] - samples[i + lag];
                x * x
            })
            .sum::<f64>();
        raw[lag] = d;
        sum += d;
        norm[lag] = if sum > 0. { d * lag as f64 / sum } else { 1. };
    }
    let mut best = min;
    let mut i = min;
    while i <= max {
        if norm[i] < 0.12 {
            while i < max && norm[i + 1] < norm[i] {
                i += 1;
            }
            best = i;
            break;
        }
        if norm[i] < norm[best] {
            best = i;
        }
        i += 1;
    }
    let conf = 1. - norm[best];
    if conf < 0.85 {
        return (f32::NAN, conf as f32);
    }
    let (a, b, c) = (raw[best - 1], raw[best], raw[best + 1]);
    let curvature = a - 2. * b + c;
    let delta = if curvature > 0. {
        (0.5 * (a - c) / curvature).clamp(-0.5, 0.5)
    } else {
        0.
    };
    let mut period = best as f64 + delta;
    // Refine progressively longer multiples. Each pass keeps the previously
    // selected octave and reduces fractional-sample bias before the next pass.
    for target_cycles in [4f64, 16., 64.] {
        let cycles = target_cycles.min(((max - 1) as f64 / period).floor().max(1.));
        if cycles <= 1. {
            continue;
        }
        let center = period * cycles;
        let radius = period * 0.35;
        let from = (center - radius).ceil().max(2.) as usize;
        let to = (center + radius).floor().min(max as f64) as usize;
        if let Some(refined) = (from..=to).min_by(|&a, &b| raw[a].total_cmp(&raw[b])) {
            let (a, b, c) = (raw[refined - 1], raw[refined], raw[refined + 1]);
            let curvature = a - 2. * b + c;
            let fraction = if curvature > 0. {
                (0.5 * (a - c) / curvature).clamp(-0.5, 0.5)
            } else {
                0.
            };
            period = (refined as f64 + fraction) / cycles;
        }
    }
    let hz = rate / period;
    if !(27.5..=2000.).contains(&hz) {
        (f32::NAN, 0.)
    } else {
        (hz as f32, conf as f32)
    }
}
pub struct Detector {
    shared: Arc<Readings>,
    worker: Option<JoinHandle<()>>,
    rate: f64,
    decimate: usize,
    phase: usize,
    lp: [f64; 4],
    hp: [f64; 2],
    input: i32,
    mute: f64,
}
impl Default for Detector {
    fn default() -> Self {
        let shared = Arc::new(Readings::new());
        let w = shared.clone();
        let worker = thread::spawn(move || {
            let mut ring = [0.; FRAME];
            let (mut at, mut filled, mut hop) = (0, 0, 0);
            let (mut generation, mut overflow) = (0, 0);
            while !w.stop.load(Ordering::Acquire) {
                let g = w.generation.load(Ordering::Acquire);
                let o = w.overrun.load(Ordering::Relaxed);
                if g != generation || o != overflow {
                    generation = g;
                    overflow = o;
                    at = 0;
                    filled = 0;
                    hop = 0;
                    ring.fill(0.);
                    w.read
                        .store(w.write.load(Ordering::Acquire), Ordering::Release);
                }
                let rate = f32::from_bits(w.rate.load(Ordering::Relaxed)) as f64;
                let mut worked = false;
                while let Some(x) = w.pop() {
                    worked = true;
                    ring[at] = x;
                    at = (at + 1) % FRAME;
                    filled = (filled + 1).min(FRAME);
                    hop += 1;
                    if filled == FRAME && hop >= rate as usize / 25 {
                        hop = 0;
                        let (hz, conf) = estimate(
                            &ring,
                            at,
                            rate,
                            f32::from_bits(w.gate.load(Ordering::Relaxed)) as f64,
                        );
                        if w.generation.load(Ordering::Acquire) == generation {
                            w.hz.store(hz.to_bits(), Ordering::Relaxed);
                            w.confidence.store(conf.to_bits(), Ordering::Relaxed);
                            w.updated
                                .store(w.clock.elapsed().as_millis() as u64, Ordering::Release);
                            w.published.store(generation, Ordering::Release);
                        }
                    }
                    if w.stop.load(Ordering::Acquire)
                        || w.generation.load(Ordering::Acquire) != generation
                    {
                        break;
                    }
                }
                if !worked {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        });
        Self {
            shared,
            worker: Some(worker),
            rate: 48000.,
            decimate: 2,
            phase: 0,
            lp: [0.; 4],
            hp: [0.; 2],
            input: 0,
            mute: 1.,
        }
    }
}
impl Detector {
    #[cfg(test)]
    pub(crate) fn publish_test(&self, hz: f32) {
        self.shared.hz.store(hz.to_bits(), Ordering::Relaxed);
        self.shared.updated.store(
            self.shared.clock.elapsed().as_millis() as u64,
            Ordering::Release,
        );
        self.shared.published.store(
            self.shared.generation.load(Ordering::Acquire),
            Ordering::Release,
        );
    }
    #[cfg(test)]
    pub(crate) fn reading(&self) -> f32 {
        self.shared.reading()
    }
}
impl Drop for Detector {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
pub(crate) fn note(hz: f32, a4: f32) -> (String, f32) {
    let midi = 69. + 12. * (hz / a4).log2();
    let key = midi.round() as i32;
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    (
        format!(
            "{}{}",
            names[key.rem_euclid(12) as usize],
            key.div_euclid(12) - 1
        ),
        (midi - key as f32) * 100.,
    )
}
struct Panel {
    p: Arc<FxParams>,
    shared: Arc<Readings>,
}
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        let hz = self.shared.reading();
        let valid = hz.is_finite();
        let (label, cents) = if valid {
            note(hz, self.p.v(0) as f32 / 10.)
        } else {
            ("--".into(), 0.)
        };
        let color = if valid && cents.abs() <= 3. {
            Color::rgb(0x50c486)
        } else {
            Color::rgb(0x28aaf0)
        };
        c.text(Font::new(46), 26., 182., color, &label);
        text(
            c,
            230.,
            193.,
            &if valid {
                format!("{hz:.2} Hz   {cents:+.1} cents")
            } else {
                "Play one sustained note".into()
            },
        );
        text(
            c,
            230.,
            218.,
            "Chromatic • A4 calibration above • single notes only",
        );
        c.fill_rrect(Rect::new(28., 271., 604., 8.), 4., Color::rgb(0x1c1c1c));
        c.line(330., 257., 330., 291., 2., Color::rgb(0x888888));
        for i in 0..=10 {
            let x = 28. + i as f32 * 60.4;
            c.line(x, 268., x, 282., 1., Color::rgb(0x555555));
        }
        if valid {
            let x = 330. + cents.clamp(-50., 50.) * 6.04;
            c.fill_circle(x, 275., 7., color);
        }
        text(c, 28., 299., "-50 cents / FLAT");
        text(c, 291., 299., "IN TUNE");
        text(c, 524., 299., "SHARP / +50");
        text(
            c,
            16.,
            338.,
            "27.5–2000 Hz • analysis does not change pitch • zero processing latency",
        );
        text(
            c,
            16.,
            361.,
            "Mute silences this insert's output; the tuner keeps measuring its input.",
        );
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}
impl Unit for Detector {
    const SPEC: &'static Spec = &specs::TUNER;
    const ID: &'static str = "com.mlstudio.tuner";
    const CLASS: [u8; 16] = *b"MLStudioTuner001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        self.decimate = (self.rate / 24000.).ceil() as usize;
        self.shared
            .rate
            .store((rate / self.decimate as f32).to_bits(), Ordering::Relaxed);
        self.reset(p);
    }
    fn reset(&mut self, p: &FxParams) {
        self.phase = 0;
        self.lp = [0.; 4];
        self.hp = [0.; 2];
        self.input = p.v(2);
        self.mute = if p.v(3) == 0 { 1. } else { 0. };
        self.shared.invalidate();
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let mode = p.v(2);
        if mode != self.input {
            self.input = mode;
            self.lp = [0.; 4];
            self.hp = [0.; 2];
            self.shared.invalidate();
        }
        self.shared
            .gate
            .store(10f32.powf(p.v(1) as f32 / 20.).to_bits(), Ordering::Relaxed);
        let alpha = 1. - (-std::f64::consts::TAU * 3000f64.min(self.rate * 0.3) / self.rate).exp();
        let hp = (-std::f64::consts::TAU * 10. / self.rate).exp();
        let mute = if p.v(3) == 0 { 1. } else { 0. };
        let step = 1. / (self.rate * 0.01);
        for i in 0..l.len() {
            let x = match mode {
                0 => l[i] as f64,
                1 => r[i] as f64,
                _ => (l[i] as f64 + r[i] as f64) * 0.5,
            };
            let x = if x.is_finite() { x } else { 0. };
            let high = x - self.hp[0] + hp * self.hp[1];
            self.hp = [x, high];
            let mut filtered = high;
            for state in &mut self.lp {
                *state += alpha * (filtered - *state);
                filtered = *state;
            }
            self.phase += 1;
            if self.phase >= self.decimate {
                self.phase = 0;
                self.shared.push(filtered as f32);
            }
            self.mute += (mute - self.mute).clamp(-step, step);
            if self.mute != 1. {
                l[i] *= self.mute as f32;
                r[i] *= self.mute as f32;
            }
        }
    }
    fn panel(&self, p: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel {
            p,
            shared: self.shared.clone(),
        })
    }
}
