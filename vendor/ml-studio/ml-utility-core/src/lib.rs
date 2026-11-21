pub mod frequency;
pub mod riser;
pub mod multidelay;
mod ring;
mod specs;
pub mod tuner;
use os_ui::{Canvas, Color, Font, Rect};
fn text(c: &mut Canvas, x: f32, y: f32, s: &str) {
    c.text(Font::new(12), x, y, Color::rgb(0xdddddd), s);
}
#[cfg(test)]
mod tests;
use ml_small_fx::{
    Shared, Spec,
    editor::{Extension, FxEditor},
    params::FxParams,
};
use nih_plug::prelude::*;
use std::{num::NonZeroU32, sync::Arc};
pub trait Unit: Default + Send + 'static {
    const SPEC: &'static Spec;
    const ID: &'static str;
    const CLASS: [u8; 16];
    const LATENCY: u32 = 0;
    fn prepare(&mut self, rate: f32, p: &FxParams);
    fn reset(&mut self, p: &FxParams);
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams);
    fn panel(&self, p: Arc<FxParams>) -> Arc<dyn Extension>;
    fn tempo(&mut self, _bpm: Option<f64>) {}
}
pub struct Fx<T: Unit> {
    pub(crate) params: Arc<FxParams>,
    pub(crate) dsp: T,
}
impl<T: Unit> Default for Fx<T> {
    fn default() -> Self {
        Self {
            params: Arc::new(FxParams::new(T::SPEC)),
            dsp: T::default(),
        }
    }
}
impl<T: Unit> Plugin for Fx<T> {
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
        shared.extension = Some(self.dsp.panel(self.params.clone()));
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
        self.dsp.prepare(c.sample_rate, &self.params);
        ctx.set_latency_samples(T::LATENCY);
        true
    }
    fn reset(&mut self) {
        self.dsp.reset(&self.params);
    }
    fn process(
        &mut self,
        b: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        ctx: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.dsp.tempo(ctx.transport().tempo);
        let c = b.as_slice();
        if c.len() >= 2 {
            let (l, r) = c.split_at_mut(1);
            self.dsp.audio(l[0], r[0], &self.params);
        }
        ProcessStatus::KeepAlive
    }
}
impl<T: Unit> ClapPlugin for Fx<T> {
    const CLAP_ID: &'static str = T::ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(T::SPEC.about);
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}
impl<T: Unit> Vst3Plugin for Fx<T> {
    const VST3_CLASS_ID: [u8; 16] = T::CLASS;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
