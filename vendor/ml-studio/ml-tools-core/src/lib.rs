// SPDX-License-Identifier: GPL-3.0-or-later
//! Native ML stock wrappers. Donor DSP provenance is recorded in README.md.
use ml_small_fx::{
    Shared, Spec,
    editor::{Extension, FxEditor},
    params::FxParams,
};
use nih_plug::prelude::*;
use os_ui::{Canvas, Color, Font, Rect};
use std::{
    marker::PhantomData,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};
pub mod deesser;
pub mod meter;
pub mod pitch;
mod specs;
#[cfg(test)]
mod tests;

pub struct Readings {
    values: [AtomicU32; 16],
    reset: AtomicU32,
}
impl Default for Readings {
    fn default() -> Self {
        Self {
            values: std::array::from_fn(|_| AtomicU32::new(f32::NEG_INFINITY.to_bits())),
            reset: AtomicU32::new(0),
        }
    }
}
impl Readings {
    pub fn get(&self, i: usize) -> f32 {
        f32::from_bits(self.values[i].load(Ordering::Relaxed))
    }
    pub fn set(&self, i: usize, v: f32) {
        self.values[i].store(v.to_bits(), Ordering::Relaxed);
    }
    fn clear_meter(&self) {
        for i in 0..9 {
            self.set(i, f32::NEG_INFINITY);
        }
        self.set(9, 0.);
        self.set(10, 0.);
    }
}
pub trait Processor: Send {
    fn new(rate: u32, data: Arc<Readings>) -> Self
    where
        Self: Sized;
    fn latency(&self) -> u32 {
        0
    }
    fn reset(&mut self, params: &FxParams);
    fn process(&mut self, l: &mut [f32], r: &mut [f32], params: &FxParams);
}
pub trait Tool: Send + Sync + 'static {
    type Dsp: Processor;
    const SPEC: &'static Spec;
    const ID: &'static str;
    const CLASS: [u8; 16];
    const METER: bool = false;
    fn paint(c: &mut Canvas, data: &Readings);
}
pub struct Stock<T: Tool> {
    params: Arc<FxParams>,
    data: Arc<Readings>,
    dsp: Option<T::Dsp>,
    kind: PhantomData<T>,
}
impl<T: Tool> Default for Stock<T> {
    fn default() -> Self {
        Self {
            params: Arc::new(FxParams::new(T::SPEC)),
            data: Arc::default(),
            dsp: None,
            kind: PhantomData,
        }
    }
}
struct Panel<T: Tool> {
    data: Arc<Readings>,
    kind: PhantomData<T>,
}
impl<T: Tool> Extension for Panel<T> {
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if os_plugkit::visibility::stock_dsp_controls() {
            T::paint(c, &self.data);
        }
    }
    fn click(&self, x: f32, y: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        if T::METER
            && os_plugkit::visibility::stock_dsp_controls()
            && Rect::new(16., 286., 120., 28.).contains(x, y)
        {
            self.data.reset.fetch_add(1, Ordering::Release);
            self.data.clear_meter();
            true
        } else {
            false
        }
    }
}
impl<T: Tool> Plugin for Stock<T> {
    const NAME: &'static str = T::SPEC.name;
    const VENDOR: &'static str = "ML Studio";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let mut shared = Shared::default();
        shared.extension = Some(Arc::new(Panel::<T> {
            data: self.data.clone(),
            kind: PhantomData,
        }));
        Some(Box::new(FxEditor::new(
            T::SPEC,
            self.params.clone(),
            Arc::new(shared),
        )))
    }
    fn initialize(
        &mut self,
        _: &AudioIOLayout,
        c: &BufferConfig,
        ctx: &mut impl InitContext<Self>,
    ) -> bool {
        if !(8000.0..=192000.0).contains(&c.sample_rate) {
            return false;
        }
        let mut dsp = T::Dsp::new(c.sample_rate.round() as u32, self.data.clone());
        ctx.set_latency_samples(dsp.latency());
        dsp.reset(&self.params);
        self.dsp = Some(dsp);
        true
    }
    fn reset(&mut self) {
        if let Some(d) = &mut self.dsp {
            d.reset(&self.params);
        }
    }
    fn process(
        &mut self,
        b: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        _: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Some(d) = &mut self.dsp {
            let channels = b.as_slice();
            if channels.len() >= 2 {
                let (l, r) = channels.split_at_mut(1);
                d.process(l[0], r[0], &self.params);
            }
        }
        ProcessStatus::KeepAlive
    }
}
impl<T: Tool> ClapPlugin for Stock<T> {
    const CLAP_ID: &'static str = T::ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(T::SPEC.about);
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
    ];
}
impl<T: Tool> Vst3Plugin for Stock<T> {
    const VST3_CLASS_ID: [u8; 16] = T::CLASS;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
pub fn db(v: f64) -> f32 {
    if v > 1e-12 {
        (20. * v.log10()) as f32
    } else {
        f32::NEG_INFINITY
    }
}
fn readout(c: &mut Canvas, x: f32, y: f32, label: &str, value: f32, unit: &str) {
    c.text(Font::new(11), x, y, Color::rgb(0xaaaaaa), label);
    let text = if value.is_finite() {
        if unit.is_empty() {
            format!("{value:+.2}")
        } else {
            format!("{value:.1} {unit}")
        }
    } else {
        "—".into()
    };
    let color = if value >= 0. && matches!(unit, "dBFS" | "dBTP") {
        Color::rgb(0xe05a58)
    } else {
        Color::rgb(0xeeeeee)
    };
    c.text(Font::new(16), x, y + 19., color, &text);
}
