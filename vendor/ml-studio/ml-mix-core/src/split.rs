// Three-way LR4 topology follows ChowDSP CrossoverFilter (GPLv3):
// split low/rest, split rest into mid/high, all-pass-align low at the upper split.
use crate::*;
pub type Split = Fx<Splitter>;
#[derive(Default, Clone, Copy)]
struct Svf {
    z1: f64,
    z2: f64,
}
impl Svf {
    fn run(&mut self, x: f64, g: f64) -> (f64, f64) {
        let k = std::f64::consts::SQRT_2;
        let a = 1. / (1. + g * (g + k));
        let v1 = a * (self.z1 + g * (x - self.z2));
        let v2 = self.z2 + g * v1;
        self.z1 = 2. * v1 - self.z1;
        self.z2 = 2. * v2 - self.z2;
        (v2, x - k * v1 - v2)
    }
}
#[derive(Default, Clone, Copy)]
struct Lr {
    first: Svf,
    low: Svf,
    high: Svf,
}
impl Lr {
    fn run(&mut self, x: f64, g: f64) -> (f64, f64) {
        let (l, h) = self.first.run(x, g);
        (self.low.run(l, g).0, self.high.run(h, g).1)
    }
}
pub struct Splitter {
    rate: f64,
    filters: [[Lr; 3]; 2],
    smooth: [f64; 6],
    selection: [f64; 3],
}
impl Default for Splitter {
    fn default() -> Self {
        Self {
            rate: 48000.,
            filters: Default::default(),
            smooth: [0.; 6],
            selection: [1.; 3],
        }
    }
}
impl Splitter {
    fn targets(&self, p: &FxParams) -> [f64; 6] {
        let hi = (p.v(1) as f64).min(self.rate * 0.45).max(40.);
        let lo = (p.v(0) as f64).min(hi * 0.8).max(20.);
        [
            lo,
            hi,
            10f64.powf(p.v(2) as f64 / 200.),
            10f64.powf(p.v(3) as f64 / 200.),
            10f64.powf(p.v(4) as f64 / 200.),
            10f64.powf(p.v(5) as f64 / 200.),
        ]
    }
    /// Process one channel using the two prewarped crossover coefficients.
    pub fn bands(&mut self, ch: usize, x: f64, g: [f64; 2]) -> [f64; 3] {
        let (l, h) = self.filters[ch][0].run(x, g[0]);
        let (m, h) = self.filters[ch][1].run(h, g[1]);
        let (a, b) = self.filters[ch][2].run(l, g[1]);
        [a + b, m, h]
    }
}
pub(crate) struct Panel {
    pub p: Arc<FxParams>,
}
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        text(c, 170., 187., "All sums the three phase-aligned bands.");
        text(
            c,
            170.,
            211.,
            "For separate effects: send to three parallel Mixer tracks,",
        );
        text(
            c,
            170.,
            233.,
            "load ML Split on each, then choose Low, Mid and High.",
        );
        text(
            c,
            170.,
            255.,
            "Use identical crossovers and disable the original dry route.",
        );
        let lo = self.p.v(0).min((self.p.v(1) as f64 * 0.8) as i32);
        if lo != self.p.v(0) {
            text(
                c,
                16.,
                291.,
                "Low crossover is limited below High to keep the bands ordered.",
            );
        }
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}
impl Unit for Splitter {
    const SPEC: &'static Spec = &specs::SPLIT;
    const ID: &'static str = "com.mlstudio.split";
    const CLASS: [u8; 16] = *b"MLStudioSplit001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        self.reset(p)
    }
    fn reset(&mut self, p: &FxParams) {
        self.filters = Default::default();
        self.smooth = self.targets(p);
        self.selection = std::array::from_fn(|i| {
            if p.v(6) == 0 || p.v(6) == i as i32 + 1 {
                1.
            } else {
                0.
            }
        });
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let target = self.targets(p);
        let coeff = 1. - (-1. / (0.02 * self.rate)).exp();
        let select: [f64; 3] = std::array::from_fn(|i| {
            if p.v(6) == 0 || p.v(6) == i as i32 + 1 {
                1.
            } else {
                0.
            }
        });
        for i in 0..l.len() {
            for k in 0..6 {
                self.smooth[k] += coeff * (target[k] - self.smooth[k]);
            }
            for k in 0..3 {
                self.selection[k] += coeff * (select[k] - self.selection[k]);
            }
            let g = [
                (std::f64::consts::PI * self.smooth[0] / self.rate).tan(),
                (std::f64::consts::PI * self.smooth[1] / self.rate).tan(),
            ];
            for ch in 0..2 {
                let x = if ch == 0 { l[i] } else { r[i] };
                let bands = self.bands(ch, if x.is_finite() { x as f64 } else { 0. }, g);
                let y = (0..3)
                    .map(|b| bands[b] * self.smooth[b + 2] * self.selection[b])
                    .sum::<f64>()
                    * self.smooth[5];
                if ch == 0 {
                    l[i] = y as f32
                } else {
                    r[i] = y as f32
                }
            }
        }
    }
    fn panel(p: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel { p })
    }
}
