use crate::*;
use std::sync::Mutex;
pub type Waveshaper = Fx<Shaper>;
const GRAPH: Rect = Rect::new(28., 218., 600., 184.);
pub(crate) struct Panel {
    pub p: Arc<FxParams>,
    drag: Mutex<Option<(usize, Option<Arc<dyn GuiContext>>, i32)>>,
}
impl Panel {
    pub fn new(p: Arc<FxParams>) -> Self {
        Self {
            p,
            drag: Mutex::new(None),
        }
    }
    fn value(&self, index: usize, v: i32, ctx: Option<&Arc<dyn GuiContext>>) {
        let param = &self.p.p[index];
        self.p.live.set(param.as_ptr(), param.preview_normalized(v));
        if let Some(c) = ctx {
            ParamSetter::new(c.as_ref()).set_parameter(param, v);
        }
    }
    fn preset(&self, kind: usize, ctx: Option<&Arc<dyn GuiContext>>) {
        let values: [i32; 9] = std::array::from_fn(|i| {
            let x = (i as f64 - 4.) / 4.;
            match kind {
                0 => (x * 1000.).round() as i32,
                1 => ((2. * x).tanh() / 2f64.tanh() * 1000.).round() as i32,
                2 => (2. * x).clamp(-1., 1.).mul_add(1000., 0.).round() as i32,
                _ => (x * std::f64::consts::TAU).sin().mul_add(1000., 0.).round() as i32,
            }
        });
        if let Some(c) = ctx {
            let mut state = c.get_state();
            for (i, v) in values.into_iter().enumerate() {
                state.params.insert(
                    format!("p{i}"),
                    nih_plug::wrapper::state::ParamValue::I32(v),
                );
            }
            self.p.live.clear();
            c.set_state(state);
        } else {
            for (i, v) in values.into_iter().enumerate() {
                self.value(i + 5, v, None);
            }
        }
    }
    fn finish(&self, cancel: bool) {
        if let Some((i, ctx, old)) = self.drag.lock().unwrap().take() {
            if cancel {
                self.value(i, old, ctx.as_ref());
            }
            if let Some(ctx) = ctx {
                ParamSetter::new(ctx.as_ref()).end_set_parameter(&self.p.p[i]);
            }
        }
    }
}
impl Extension for Panel {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        for (i, label) in ["Linear", "Soft", "Hard", "Fold"].iter().enumerate() {
            button(
                c,
                Rect::new(28. + i as f32 * 120., 169., 110., 28.),
                label,
                false,
            );
        }
        c.fill_rect(GRAPH, Color::rgb(0x1d1d1d));
        for i in 0..=8 {
            let x = GRAPH.x + GRAPH.w * i as f32 / 8.;
            c.line(x, GRAPH.y, x, GRAPH.y + GRAPH.h, 1., Color::rgb(0x333333));
        }
        c.line(
            GRAPH.x,
            GRAPH.y + GRAPH.h / 2.,
            GRAPH.x + GRAPH.w,
            GRAPH.y + GRAPH.h / 2.,
            1.,
            Color::rgb(0x777777),
        );
        let mut last = None;
        for i in 0..9 {
            let x = GRAPH.x + GRAPH.w * i as f32 / 8.;
            let y = GRAPH.y + GRAPH.h * (1. - self.p.v(i + 5) as f32 / 1000.) / 2.;
            if let Some((px, py)) = last {
                c.line(px, py, x, y, 2., Color::rgb(0x24a9ef));
            }
            c.fill_circle(x, y, 5., Color::rgb(0x24a9ef));
            last = Some((x, y));
        }
        text(
            c,
            28.,
            413.,
            "Drag a point vertically • presets replace the curve • Undo restores edits",
        );
        text(
            c,
            28.,
            435.,
            "4× oversampling • fixed 32-sample latency • dry/wet delay aligned",
        );
    }
    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return false;
        }
        for i in 0..4 {
            if Rect::new(28. + i as f32 * 120., 169., 110., 28.).contains(x, y) {
                self.preset(i, ctx);
                return true;
            }
        }
        if GRAPH.contains(x, y) {
            let i = (((x - GRAPH.x) / GRAPH.w * 8.).round() as usize).min(8) + 5;
            let old = self.p.v(i);
            if let Some(c) = ctx {
                ParamSetter::new(c.as_ref()).begin_set_parameter(&self.p.p[i]);
            }
            *self.drag.lock().unwrap() = Some((i, ctx.cloned(), old));
            self.moved(x, y);
            return true;
        }
        false
    }
    fn dragging(&self) -> bool {
        self.drag.lock().unwrap().is_some()
    }
    fn moved(&self, _: f32, y: f32) -> bool {
        let drag = self.drag.lock().unwrap();
        if let Some((i, ctx, _)) = drag.as_ref() {
            let v = ((1. - 2. * (y - GRAPH.y) / GRAPH.h) * 1000.)
                .round()
                .clamp(-1000., 1000.) as i32;
            self.value(*i, v, ctx.as_ref());
            true
        } else {
            false
        }
    }
    fn release(&self, _: Option<&Arc<dyn GuiContext>>) -> bool {
        let had = self.dragging();
        self.finish(false);
        had
    }
    fn character(&self, c: char) -> bool {
        if c == '\u{1b}' && self.dragging() {
            self.finish(true);
            true
        } else {
            false
        }
    }
    fn editor_closed(&self) {
        self.finish(false);
    }
}
// Linear-phase polyphase FIR oversampling: 129 taps up + down at 4x = 32 base-rate samples.
struct Over {
    up: [f64; 33],
    down: [f64; 129],
    u: usize,
    d: usize,
}
impl Default for Over {
    fn default() -> Self {
        Self {
            up: [0.; 33],
            down: [0.; 129],
            u: 0,
            d: 0,
        }
    }
}
pub struct Shaper {
    rate: f64,
    taps: [f64; 129],
    over: [Over; 2],
    dry: [[f32; 32]; 2],
    at: usize,
    smooth: [f64; 14],
    hp: [[f64; 2]; 2],
}
impl Default for Shaper {
    fn default() -> Self {
        Self {
            rate: 48000.,
            taps: [0.; 129],
            over: Default::default(),
            dry: [[0.; 32]; 2],
            at: 0,
            smooth: [0.; 14],
            hp: [[0.; 2]; 2],
        }
    }
}
pub(crate) fn curve(points: &[f64], x: f64) -> f64 {
    let t = (x.clamp(-1., 1.) + 1.) * 4.;
    let i = (t.floor() as usize).min(7);
    points[i] + (points[i + 1] - points[i]) * (t - i as f64)
}
impl Unit for Shaper {
    const SPEC: &'static Spec = &specs::SHAPER;
    const ID: &'static str = "com.mlstudio.waveshaper";
    const CLASS: [u8; 16] = *b"MLWaveshaper0001";
    const LATENCY: u32 = 32;
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate as f64;
        for (i, w) in self.taps.iter_mut().enumerate() {
            let x = i as f64 - 64.;
            let h = if x == 0. {
                0.24
            } else {
                (std::f64::consts::PI * 0.24 * x).sin() / (std::f64::consts::PI * x)
            };
            *w = h
                * (0.42
                    + 0.5 * (std::f64::consts::PI * x / 64.).cos()
                    + 0.08 * (std::f64::consts::TAU * x / 64.).cos());
        }
        let sum: f64 = self.taps.iter().sum();
        for w in &mut self.taps {
            *w /= sum;
        }
        self.reset(p);
    }
    fn reset(&mut self, p: &FxParams) {
        self.over = Default::default();
        self.dry = [[0.; 32]; 2];
        self.hp = [[0.; 2]; 2];
        self.at = 0;
        self.smooth = std::array::from_fn(|i| match i {
            0 | 3 => 10f64.powf(p.v(i) as f64 / 200.),
            4 => p.v(i) as f64,
            _ => p.v(i) as f64 / 1000.,
        });
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let target: [f64; 14] = std::array::from_fn(|i| match i {
            0 | 3 => 10f64.powf(p.v(i) as f64 / 200.),
            4 => p.v(i) as f64,
            _ => p.v(i) as f64 / 1000.,
        });
        let smoothing = 1. - (-1. / (0.01 * self.rate)).exp();
        let dc = (-std::f64::consts::TAU * 10. / self.rate).exp();
        for i in 0..l.len() {
            for k in 0..14 {
                self.smooth[k] += smoothing * (target[k] - self.smooth[k]);
            }
            for ch in 0..2 {
                let input = if ch == 0 { l[i] } else { r[i] };
                let input = if input.is_finite() { input } else { 0. };
                let delayed = self.dry[ch][self.at] as f64;
                self.dry[ch][self.at] = input;
                let os = &mut self.over[ch];
                os.up[os.u] = input as f64;
                let mut wet = 0.;
                for phase in 0..4 {
                    let mut up = 0.;
                    for k in 0..33 {
                        let tap = phase + 4 * k;
                        if tap < 129 {
                            up += self.taps[tap] * os.up[(os.u + 33 - k) % 33] * 4.;
                        }
                    }
                    os.down[os.d] = curve(&self.smooth[5..], up * self.smooth[0] + self.smooth[1]);
                    if phase == 0 {
                        for k in 0..129 {
                            wet += self.taps[k] * os.down[(os.d + 129 - k) % 129];
                        }
                    }
                    os.d = (os.d + 1) % 129;
                }
                os.u = (os.u + 1) % 33;
                let high = wet - self.hp[ch][0] + dc * self.hp[ch][1];
                self.hp[ch] = [wet, high];
                wet += self.smooth[4] * (high - wet);
                let output = (delayed + (wet - delayed) * self.smooth[2]) * self.smooth[3];
                if ch == 0 {
                    l[i] = output as f32
                } else {
                    r[i] = output as f32
                }
            }
            self.at = (self.at + 1) % 32;
        }
    }
    fn panel(p: Arc<FxParams>) -> Arc<dyn Extension> {
        Arc::new(Panel::new(p))
    }
}
