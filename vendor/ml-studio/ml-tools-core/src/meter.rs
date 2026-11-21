use super::*;
use ebur128::{EbuR128, Mode};
pub struct Meter;
impl Tool for Meter {
    type Dsp = Measure;
    const SPEC: &'static Spec = &specs::METER;
    const ID: &'static str = "com.mlstudio.meter";
    const CLASS: [u8; 16] = *b"MLStudioMeter001";
    const METER: bool = true;
    fn paint(c: &mut Canvas, d: &Readings) {
        for (x, base) in [(26., 0), (214., 1)] {
            readout(c, x, 42., "SAMPLE PEAK HOLD", d.get(base), "dBFS");
            readout(c, x, 100., "RMS / 400 ms", d.get(base + 2), "dBFS");
            readout(c, x, 158., "TRUE PEAK HOLD", d.get(base + 4), "dBTP");
            let rms = d.get(base + 2);
            c.fill_rrect(Rect::new(x, 145., 150., 4.), 2., Color::rgb(0x202020));
            let width = ((rms + 60.) / 60.).clamp(0., 1.) * 150.;
            if width > 0. {
                c.fill_rrect(
                    Rect::new(x, 145., width, 4.),
                    2.,
                    if rms >= 0. {
                        Color::rgb(0xe05a58)
                    } else if rms > -6. {
                        Color::rgb(0xe0b44f)
                    } else {
                        Color::rgb(0x49b78a)
                    },
                );
            }
        }
        for (y, i, label) in [
            (42., 6, "MOMENTARY / 400 ms"),
            (100., 7, "SHORT TERM / 3 s"),
            (158., 8, "INTEGRATED / GATED"),
        ] {
            readout(c, 408., y, label, d.get(i), "LUFS");
        }
        readout(c, 26., 222., "L/R CORRELATION", d.get(9), "");
        c.fill_rrect(Rect::new(26., 263., 150., 4.), 2., Color::rgb(0x202020));
        let corr = d.get(9).clamp(-1., 1.);
        if corr.is_finite() {
            c.fill_rect(
                Rect::new(101. + corr.min(0.) * 75., 263., corr.abs() * 75., 4.),
                if corr < 0. {
                    Color::rgb(0xe05a58)
                } else {
                    Color::rgb(0x49b78a)
                },
            );
        }
        c.line(101., 260., 101., 270., 1., Color::rgb(0xaaaaaa));
        readout(c, 214., 222., "MEASUREMENT", d.get(10), "s");
        os_ui::kit::inst::text_button(
            c,
            Rect::new(16., 286., 120., 28.),
            "Reset meters",
            os_ui::kit::State::NORMAL,
        );
        c.text(
            Font::new(11),
            152.,
            294.,
            Color::rgb(0xaaaaaa),
            "Measures here in the chain. Audio passes through unchanged.",
        );
    }
}
pub struct Measure {
    ebu: EbuR128,
    data: Arc<Readings>,
    rate: u32,
    window: Vec<[f64; 3]>,
    pos: usize,
    sums: [f64; 3],
    frames: u64,
    tick: usize,
    reset_seen: u32,
}
impl Processor for Measure {
    fn new(rate: u32, data: Arc<Readings>) -> Self {
        // Histogram history avoids an ever-growing history queue and heap work in process().
        let ebu = EbuR128::new(
            2,
            rate,
            Mode::M | Mode::S | Mode::I | Mode::TRUE_PEAK | Mode::HISTOGRAM,
        )
        .expect("validated stereo sample rate");
        Self {
            ebu,
            data,
            rate,
            window: vec![[0.; 3]; (rate as usize * 2 / 5).max(1)],
            pos: 0,
            sums: [0.; 3],
            frames: 0,
            tick: 0,
            reset_seen: 0,
        }
    }
    fn reset(&mut self, _: &FxParams) {
        self.ebu.reset();
        self.window.fill([0.; 3]);
        self.pos = 0;
        self.sums = [0.; 3];
        self.frames = 0;
        self.tick = 0;
        self.reset_seen = self.data.reset.load(Ordering::Acquire);
        for i in 0..9 {
            self.data.set(i, f32::NEG_INFINITY);
        }
        self.data.set(9, 0.);
        self.data.set(10, 0.);
    }
    fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        if self.reset_seen != self.data.reset.load(Ordering::Acquire) {
            self.reset(p);
        }
        // Keep corrupt upstream samples out of the measurement while leaving audio untouched.
        let mut clean_l = [0f32; 256];
        let mut clean_r = [0f32; 256];
        for (ll, rr) in l.chunks(256).zip(r.chunks(256)) {
            for i in 0..ll.len() {
                clean_l[i] = if ll[i].is_finite() { ll[i] } else { 0. };
                clean_r[i] = if rr[i].is_finite() { rr[i] } else { 0. };
            }
            let _ = self
                .ebu
                .add_frames_planar_f32(&[&clean_l[..ll.len()], &clean_r[..ll.len()]]);
            for i in 0..ll.len() {
                let a = f64::from(clean_l[i]);
                let b = f64::from(clean_r[i]);
                let v = [a * a, b * b, a * b];
                for k in 0..3 {
                    self.sums[k] += v[k] - self.window[self.pos][k];
                }
                self.window[self.pos] = v;
                self.pos = (self.pos + 1) % self.window.len();
                self.frames += 1;
            }
            self.tick += ll.len();
            if self.tick >= self.rate as usize / 20 {
                self.tick = 0;
                self.publish();
            }
        }
    }
}
impl Measure {
    fn publish(&self) {
        for ch in 0..2 {
            self.data
                .set(ch, db(self.ebu.sample_peak(ch as u32).unwrap_or(0.)));
            self.data.set(
                ch + 2,
                db((self.sums[ch].max(0.) / self.window.len() as f64).sqrt()),
            );
            self.data
                .set(ch + 4, db(self.ebu.true_peak(ch as u32).unwrap_or(0.)));
        }
        let m = if self.frames >= self.rate as u64 * 2 / 5 {
            self.ebu.loudness_momentary().unwrap_or(f64::NEG_INFINITY)
        } else {
            f64::NEG_INFINITY
        };
        let s = if self.frames >= self.rate as u64 * 3 {
            self.ebu.loudness_shortterm().unwrap_or(f64::NEG_INFINITY)
        } else {
            f64::NEG_INFINITY
        };
        self.data.set(6, m as f32);
        self.data.set(7, s as f32);
        self.data.set(
            8,
            self.ebu.loudness_global().unwrap_or(f64::NEG_INFINITY) as f32,
        );
        let denom = (self.sums[0].max(0.) * self.sums[1].max(0.)).sqrt();
        self.data.set(
            9,
            if denom > 1e-20 {
                (self.sums[2] / denom).clamp(-1., 1.) as f32
            } else {
                0.
            },
        );
        self.data.set(10, self.frames as f32 / self.rate as f32);
    }
}
