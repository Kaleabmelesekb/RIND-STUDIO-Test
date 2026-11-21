// Rust adaptation of NULL Riser, Copyright (c) 2026 Christos Koulaxizis, MIT.
// Pinned original DSP, presets and licence: ../upstream/null-riser/.
use crate::*;

pub type Riser = Fx<BuildUp>;

#[derive(Default, Clone, Copy)]
struct Svf {
    g: f64,
    k: f64,
    s1: f64,
    s2: f64,
}
impl Svf {
    fn set(&mut self, frequency: f64, q: f64, rate: f64) {
        self.g = (std::f64::consts::PI * frequency.min(0.45 * rate) / rate).tan();
        self.k = 1. / q;
    }
    fn tick(&mut self, input: f64) -> (f64, f64) {
        let high =
            (input - self.s2 - (self.k + self.g) * self.s1) / (1. + self.g * (self.k + self.g));
        let band = self.g * high + self.s1;
        let low = self.g * band + self.s2;
        self.s1 = 2. * band - self.s1;
        self.s2 = 2. * low - self.s2;
        (high, self.k * band)
    }
}

pub struct BuildUp {
    rate: f64,
    smoothing: f64,
    amount: f64,
    seed: f64,
    high: [Svf; 2],
    noise: [Svf; 2],
    delay: [Vec<f64>; 2],
    position: [usize; 2],
}
impl Default for BuildUp {
    fn default() -> Self {
        Self {
            rate: 48000.,
            smoothing: 0.,
            amount: 0.,
            seed: 12345.,
            high: [Svf::default(); 2],
            noise: [Svf::default(); 2],
            delay: Default::default(),
            position: [0; 2],
        }
    }
}
impl BuildUp {
    fn random(&mut self) -> f64 {
        self.seed *= 16807.;
        self.seed -= (self.seed / 2147483647.).floor() * 2147483647.;
        self.seed / 1073741824. - 1.
    }
}

struct Panel;
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        text(c, 228., 66., "ONE KNOB. ALL THE TENSION.");
        text(c, 228., 94., "Resonant high-pass sweep");
        text(c, 228., 116., "Rising noise / stereo width / diffuse wash");
        text(c, 228., 150., "Automate Intensity up, then drop to 0%.");
        text(c, 228., 172., "0% settles to exact dry audio.");
        text(
            c,
            16.,
            218.,
            "Based on NULL Riser / Christos Koulaxizis / MIT",
        );
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}

impl Unit for BuildUp {
    const SPEC: &'static Spec = &specs::RISER;
    const ID: &'static str = "com.mlstudio.riser";
    const CLASS: [u8; 16] = *b"MLRiser000000001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        self.smoothing = (-1. / (0.03 * self.rate)).exp();
        self.delay =
            [0.083, 0.097].map(|seconds| vec![0.; (seconds * self.rate).floor().max(1.) as usize]);
        self.reset(p);
    }
    fn reset(&mut self, _: &FxParams) {
        self.amount = 0.;
        self.seed = 12345.;
        self.high = [Svf::default(); 2];
        self.noise = [Svf::default(); 2];
        self.position = [0; 2];
        for d in &mut self.delay {
            d.fill(0.);
        }
    }
    fn audio(&mut self, left: &mut [f32], right: &mut [f32], p: &FxParams) {
        let target = (p.v(0) as f64 / 100.).clamp(0., 1.);
        // Preserve NULL Riser's block-rate coefficients and 30 ms sample smoothing.
        let squared = self.amount * self.amount;
        let cutoff = 20. * 125f64.powf(self.amount);
        for filter in &mut self.high {
            filter.set(cutoff, 0.707 + 2.3 * squared, self.rate);
        }
        for filter in &mut self.noise {
            filter.set(cutoff * 2. + 200., 1.5, self.rate);
        }
        let noise = 0.35 * squared;
        let width = 10f64.powf(6. * self.amount / 20.);
        let wash = 0.35 * self.amount;
        let feedback = 0.55 * self.amount;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let input = [*l, *r].map(|x| if x.is_finite() { x as f64 } else { 0. });
            self.amount = self.smoothing * self.amount + (1. - self.smoothing) * target;
            let output = if self.amount < 0.0005 && target == 0. {
                self.amount = 0.;
                input
            } else {
                let random = [self.random(), self.random()];
                let filtered: [f64; 2] = std::array::from_fn(|ch| {
                    self.high[ch].tick(input[ch]).0 + noise * self.noise[ch].tick(random[ch]).1
                });
                let mid = (filtered[0] + filtered[1]) * 0.5;
                let side = (filtered[0] - filtered[1]) * 0.5 * width;
                let wet = [mid + side, mid - side];
                let delayed: [f64; 2] = std::array::from_fn(|ch| self.delay[ch][self.position[ch]]);
                std::array::from_fn(|ch| {
                    self.delay[ch][self.position[ch]] = wet[ch] + delayed[1 - ch] * feedback;
                    self.position[ch] = (self.position[ch] + 1) % self.delay[ch].len();
                    input[ch]
                        + (self.amount * 20.).min(1.) * (wet[ch] + wash * delayed[ch] - input[ch])
                })
            };
            *l = output[0] as f32;
            *r = output[1] as f32;
        }
    }
    fn panel(&self, _: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel)
    }
}
