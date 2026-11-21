use super::*;
use os_dsp::debess::DeBess;
pub struct DeEsser;
impl Tool for DeEsser {
    type Dsp = DeEss;
    const SPEC: &'static Spec = &specs::DEESSER;
    const ID: &'static str = "com.mlstudio.de-esser";
    const CLASS: [u8; 16] = *b"MLStudioDeEss001";
    fn paint(c: &mut Canvas, d: &Readings) {
        readout(c, 370., 54., "REDUCTION (RMS)", d.get(0), "dB");
        c.text(
            Font::new(11),
            16.,
            180.,
            Color::rgb(0xaaaaaa),
            "Listen to Removed to hear what the de-esser is taking out.",
        );
    }
}
pub struct DeEss {
    dsp: DeBess,
    rate: u32,
    data: Arc<Readings>,
    mix: f32,
    gain: f32,
    listen: f32,
    smooth: f32,
    energy: [f64; 2],
    count: usize,
}
impl Processor for DeEss {
    fn new(rate: u32, data: Arc<Readings>) -> Self {
        Self {
            dsp: DeBess::new(rate),
            rate,
            data,
            mix: 1.,
            gain: 1.,
            listen: 0.,
            smooth: 1. - (-1. / (rate as f32 * 0.005)).exp(),
            energy: [0.; 2],
            count: 0,
        }
    }
    fn reset(&mut self, p: &FxParams) {
        self.dsp = DeBess::new(self.rate);
        self.mix = p.v(1) as f32 / 1000.;
        self.gain = 10f32.powf(p.v(2) as f32 / 200.);
        self.listen = p.v(3) as f32;
        self.energy = [0.; 2];
        self.count = 0;
        self.data.set(0, 0.);
    }
    fn process(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        let amount = p.v(0) as f32 / 1000.;
        let mix = p.v(1) as f32 / 1000.;
        let gain = 10f32.powf(p.v(2) as f32 / 200.);
        let listen = p.v(3) as f32;
        let mut scratch = [0f32; 512];
        for (ll, rr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            for i in 0..ll.len() {
                scratch[i * 2] = ll[i];
                scratch[i * 2 + 1] = rr[i];
            }
            self.dsp.process(&mut scratch[..ll.len() * 2], amount);
            for i in 0..ll.len() {
                self.mix += (mix - self.mix) * self.smooth;
                self.gain += (gain - self.gain) * self.smooth;
                self.listen += (listen - self.listen) * self.smooth;
                for (sample, wet) in [
                    (&mut ll[i], scratch[2 * i]),
                    (&mut rr[i], scratch[2 * i + 1]),
                ] {
                    let dry = *sample;
                    self.energy[0] += f64::from(dry).powi(2);
                    self.energy[1] += f64::from(wet).powi(2);
                    let normal = dry + (wet - dry) * self.mix;
                    *sample = (normal + ((dry - wet) - normal) * self.listen) * self.gain;
                }
                self.count += 1;
                if self.count >= self.rate as usize / 10 {
                    let gr = if self.energy[0] > 1e-12 {
                        (-10. * (self.energy[1].max(1e-20) / self.energy[0]).log10()).max(0.) as f32
                    } else {
                        0.
                    };
                    self.data.set(0, gr);
                    self.energy = [0.; 2];
                    self.count = 0;
                }
            }
        }
    }
}
