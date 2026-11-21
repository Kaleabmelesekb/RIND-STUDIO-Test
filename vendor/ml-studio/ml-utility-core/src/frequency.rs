// Rust adaptation of Signalsmith Hilbert IIR, Geraint Luff / Signalsmith Audio, 0BSD.
use crate::*;
const COEFF: [[f64; 2]; 12] = [
    [-0.000224352093802, 0.00543499018201],
    [0.0107500557815, -0.0173890685681],
    [-0.0456795873917, 0.0229166931429],
    [0.11282500582, 0.00278413661237],
    [-0.208067578452, -0.104628958675],
    [0.28717837501, 0.33619239719],
    [-0.254675294431, -0.683033899655],
    [0.0481081835026, 0.954061589374],
    [0.227861357867, -0.891273574569],
    [-0.365411839137, 0.525088317271],
    [0.280729061131, -0.155131206606],
    [-0.0935061787728, 0.00512245855404],
];
const POLE: [[f64; 2]; 12] = [
    [-0.00495335976478, 0.0092579876872],
    [-0.017859491302, 0.0273493725543],
    [-0.0413714373155, 0.0744756910287],
    [-0.0882148408885, 0.178349677457],
    [-0.17922965812, 0.39601340223],
    [-0.338261800753, 0.829229533354],
    [-0.557688699732, 1.61298538328],
    [-0.735157736148, 2.79987398682],
    [-0.719057381172, 4.16396166128],
    [-0.517871025209, 5.29724826804],
    [-0.280197469471, 5.99598602388],
    [-0.0852751354531, 6.3048492377],
];
#[derive(Default, Clone)]
struct Hilbert {
    c: [[f64; 2]; 12],
    p: [[f64; 2]; 12],
    s: [[[f64; 2]; 12]; 2],
    direct: f64,
}
impl Hilbert {
    fn prepare(&mut self, rate: f64, gain: f64) {
        let f = (20000. / rate).min(0.46);
        self.direct = 0.000262057212648 * 2. * gain * f;
        for i in 0..12 {
            self.c[i] = COEFF[i].map(|x| x * f * gain);
            let mag = (POLE[i][0] * f).exp();
            self.p[i] = [mag * (POLE[i][1] * f).cos(), mag * (POLE[i][1] * f).sin()];
        }
        self.s = [[[0.; 2]; 12]; 2];
    }
    fn run(&mut self, ch: usize, x: [f64; 2]) -> [f64; 2] {
        let mut out = x.map(|x| x * self.direct);
        for i in 0..12 {
            let [a, b] = self.s[ch][i];
            let [c, d] = self.p[i];
            let [e, f] = self.c[i];
            let mut next = [
                a * c - b * d + x[0] * e - x[1] * f,
                a * d + b * c + x[0] * f + x[1] * e,
            ];
            for v in &mut next {
                if v.abs() < 1e-25 {
                    *v = 0.;
                }
            }
            self.s[ch][i] = next;
            out[0] += next[0];
            out[1] += next[1];
        }
        out
    }
}
pub type FrequencyShift = Fx<Shifter>;
pub struct Shifter {
    rate: f64,
    analytic: Hilbert,
    filter: Hilbert,
    phase: [f64; 2],
    smooth: [f64; 4],
}
impl Default for Shifter {
    fn default() -> Self {
        Self {
            rate: 48000.,
            analytic: Default::default(),
            filter: Default::default(),
            phase: [0.; 2],
            smooth: [0.; 4],
        }
    }
}
fn targets(p: &FxParams) -> [f64; 4] {
    let hz = p.v(0) as f64 / 10.;
    [
        hz,
        if p.v(1) == 0 { hz } else { -hz },
        p.v(2) as f64 / 1000.,
        10f64.powf(p.v(3) as f64 / 200.),
    ]
}
struct Panel;
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        text(
            c,
            16.,
            178.,
            "Moves frequencies in Hz. Opposite shifts left and right in reverse directions.",
        );
        text(
            c,
            16.,
            201.,
            "Mix 0% is exact dry. At 0 Hz, the wet signal still passes through the filters.",
        );
        text(
            c,
            16.,
            224.,
            "Content shifted below 0 Hz or beyond the filter passband is attenuated.",
        );
    }
    fn click(&self, _: f32, _: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        false
    }
}
impl Unit for Shifter {
    const SPEC: &'static Spec = &specs::FREQUENCY;
    const ID: &'static str = "com.mlstudio.frequencyshift";
    const CLASS: [u8; 16] = *b"MLFrequency00001";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        self.analytic.prepare(self.rate, 2.);
        self.filter.prepare(self.rate, 1.);
        self.reset(p);
    }
    fn reset(&mut self, p: &FxParams) {
        self.analytic.s = [[[0.; 2]; 12]; 2];
        self.filter.s = [[[0.; 2]; 12]; 2];
        self.phase = [0.; 2];
        self.smooth = targets(p);
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let target = targets(p);
        let coeff = 1. - (-1. / (self.rate * 0.02)).exp();
        for i in 0..l.len() {
            for k in 0..4 {
                self.smooth[k] += coeff * (target[k] - self.smooth[k]);
            }
            for ch in 0..2 {
                let input = if ch == 0 { l[i] } else { r[i] };
                let input = if input.is_finite() { input as f64 } else { 0. };
                let a = self.analytic.run(ch, [input, 0.]);
                let (sin, cos) = self.phase[ch].sin_cos();
                let wet = self
                    .filter
                    .run(ch, [a[0] * cos - a[1] * sin, a[0] * sin + a[1] * cos])[0];
                self.phase[ch] = (self.phase[ch]
                    + std::f64::consts::TAU * self.smooth[ch] / self.rate)
                    .rem_euclid(std::f64::consts::TAU);
                let out = (input + (wet - input) * self.smooth[2]) * self.smooth[3];
                if ch == 0 {
                    l[i] = out as f32
                } else {
                    r[i] = out as f32
                }
            }
        }
    }
    fn panel(&self, _: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel)
    }
}
