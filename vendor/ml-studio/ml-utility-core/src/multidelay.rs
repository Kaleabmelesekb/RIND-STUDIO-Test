use crate::*;
use ml_mix_core::split::Splitter;
use os_engine::effects::delay::Ring;
use std::sync::atomic::{AtomicU32, Ordering};
pub type MultibandDelay = Fx<Multidelay>;
const BEATS: [f64; 8] = [0.25, 1. / 3., 0.5, 0.75, 1., 1.5, 2., 4.];
struct Tap {
    line: [Ring; 2],
    old: f32,
    next: f32,
    fade: f32,
    hp: [[f32; 2]; 2],
}
impl Default for Tap {
    fn default() -> Self {
        Self {
            line: std::array::from_fn(|_| Ring::new(4)),
            old: 1.,
            next: 1.,
            fade: 1.,
            hp: [[0.; 2]; 2],
        }
    }
}
impl Tap {
    fn set(&mut self, time: f32, first: bool) {
        if first {
            self.old = time;
            self.next = time;
            self.fade = 1.;
        } else if self.fade >= 1. && (time - self.next).abs() > 0.01 {
            self.old = self.next;
            self.next = time;
            self.fade = 0.;
        }
    }
    fn frame(&mut self, x: [f32; 2], feedback: f32, step: f32, hp: f32) -> [f32; 2] {
        self.fade = (self.fade + step).min(1.);
        let mut out = [0.; 2];
        for ch in 0..2 {
            let a = self.line[ch].read_clean(self.old);
            let b = self.line[ch].read_clean(self.next);
            out[ch] = a + (b - a) * self.fade;
            let high = out[ch] - self.hp[ch][0] + hp * self.hp[ch][1];
            self.hp[ch] = [out[ch], high];
            let input = x[ch] + feedback * high;
            self.line[ch].write(if input.is_finite() {
                input.clamp(-16., 16.)
            } else {
                0.
            });
        }
        out
    }
}
pub struct Multidelay {
    rate: f64,
    bpm: f64,
    started: bool,
    split: Splitter,
    taps: [Tap; 3],
    smooth: [f64; 10],
    readings: Arc<[AtomicU32; 4]>,
}
impl Default for Multidelay {
    fn default() -> Self {
        Self {
            rate: 48000.,
            bpm: 120.,
            started: false,
            split: Splitter::default(),
            taps: Default::default(),
            smooth: [0.; 10],
            readings: Arc::new(std::array::from_fn(|_| AtomicU32::new(0))),
        }
    }
}
fn targets(p: &FxParams, rate: f64) -> [f64; 10] {
    let hi = (p.v(1) as f64).min(rate * 0.45);
    let lo = (p.v(0) as f64).min(hi * 0.8);
    std::array::from_fn(|i| match i {
        0 => lo,
        1 => hi,
        2 => p.v(2) as f64 / 1000.,
        3 => 10f64.powf(p.v(3) as f64 / 200.),
        4..=6 => p.v(7 + (i - 4) * 4) as f64 / 1000.,
        _ => {
            let v = p.v(8 + (i - 7) * 4);
            if v == -600 {
                0.
            } else {
                10f64.powf(v as f64 / 200.)
            }
        }
    })
}
pub(crate) fn time_ms(p: &FxParams, b: usize, bpm: f64) -> f64 {
    if p.v(4) == 0 {
        p.v(5 + b * 4) as f64
    } else {
        (60000. / bpm * BEATS[p.v(6 + b * 4) as usize]).clamp(1., 4000.)
    }
}
struct Panel {
    values: Arc<[AtomicU32; 4]>,
}
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        for (b, label) in ["LOW", "MID", "HIGH"].iter().enumerate() {
            let y = 207. + b as f32 * 142.;
            text(c, 18., y, label);
            let ms = f32::from_bits(self.values[b].load(Ordering::Relaxed));
            text(c, 18., y + 26., &format!("{ms:.1} ms"));
        }
        text(
            c,
            16.,
            620.,
            "Sync On uses Beats; Off uses Time. Effective times shown at left (maximum 4 s).",
        );
        let bpm = f32::from_bits(self.values[3].load(Ordering::Relaxed));
        text(
            c,
            16.,
            644.,
            &format!("Tempo {bpm:.1} BPM (120 fallback) • feedback capped at 90% • Mix 0% is dry"),
        );
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}
impl Unit for Multidelay {
    const SPEC: &'static Spec = &specs::MULTIDELAY;
    const ID: &'static str = "com.mlstudio.multibanddelay";
    const CLASS: [u8; 16] = *b"MLMultidelay0001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        let len = (rate as usize * 4 + 8).next_power_of_two();
        for t in &mut self.taps {
            t.line = std::array::from_fn(|_| Ring::new(len));
        }
        self.reset(p);
    }
    fn reset(&mut self, p: &FxParams) {
        self.split = Splitter::default();
        for t in &mut self.taps {
            for l in &mut t.line {
                l.clear();
            }
            t.hp = [[0.; 2]; 2];
        }
        self.started = false;
        self.smooth = targets(p, self.rate);
    }
    fn tempo(&mut self, bpm: Option<f64>) {
        self.bpm = bpm
            .filter(|v| v.is_finite() && *v > 0.)
            .unwrap_or(120.)
            .clamp(10., 1000.);
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let target = targets(p, self.rate);
        let coeff = 1. - (-1. / (self.rate * 0.02)).exp();
        for b in 0..3 {
            let ms = time_ms(p, b, self.bpm);
            self.taps[b].set((ms * self.rate / 1000.) as f32, !self.started);
            self.readings[b].store((ms as f32).to_bits(), Ordering::Relaxed);
        }
        self.readings[3].store((self.bpm as f32).to_bits(), Ordering::Relaxed);
        self.started = true;
        let step = (1. / (0.02 * self.rate)) as f32;
        let hp = (-std::f64::consts::TAU * 10. / self.rate).exp() as f32;
        for i in 0..l.len() {
            for k in 0..10 {
                self.smooth[k] += coeff * (target[k] - self.smooth[k]);
            }
            let input = [l[i], r[i]].map(|v| if v.is_finite() { v } else { 0. });
            let g = [
                (std::f64::consts::PI * self.smooth[0] / self.rate).tan(),
                (std::f64::consts::PI * self.smooth[1] / self.rate).tan(),
            ];
            let a = self.split.bands(0, input[0] as f64, g);
            let b = self.split.bands(1, input[1] as f64, g);
            let mut wet = [0.; 2];
            for band in 0..3 {
                let out = self.taps[band].frame(
                    [a[band] as f32, b[band] as f32],
                    self.smooth[4 + band] as f32,
                    step,
                    hp,
                );
                for ch in 0..2 {
                    wet[ch] += out[ch] as f64 * self.smooth[7 + band];
                }
            }
            l[i] = ((input[0] as f64 + (wet[0] - input[0] as f64) * self.smooth[2])
                * self.smooth[3]) as f32;
            r[i] = ((input[1] as f64 + (wet[1] - input[1] as f64) * self.smooth[2])
                * self.smooth[3]) as f32;
        }
    }
    fn panel(&self, _: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel {
            values: self.readings.clone(),
        })
    }
}
