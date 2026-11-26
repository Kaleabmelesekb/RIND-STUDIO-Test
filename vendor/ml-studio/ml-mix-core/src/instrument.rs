// SPDX-License-Identifier: GPL-3.0-or-later
// Honeydew's own: an instrument (MIDI in, stereo out) with the same generated ML Studio editor as the effects.
use crate::*;

pub trait Synth: Default + Send + 'static {
    const SPEC: &'static Spec;
    const ID: &'static str;
    const CLASS: [u8; 16];
    fn prepare(&mut self, rate: f32, p: &FxParams);
    fn reset(&mut self, p: &FxParams);
    fn note_on(&mut self, channel: u8, note: u8, velocity: f32, voice_id: Option<i32>);
    fn note_off(&mut self, channel: u8, note: u8, voice_id: Option<i32>);
    /// The wheel, `0.0..=1.0` with `0.5` at rest.
    fn pitch_bend(&mut self, value: f32);
    /// A control change; `value` is `0.0..=1.0`.
    fn control(&mut self, _cc: u8, _value: f32) {}
    /// Channel pressure, `0.0..=1.0`.
    fn pressure(&mut self, _value: f32) {}
    /// The host tempo in beats per minute, before each block.
    fn tempo(&mut self, _bpm: f64) {}
    /// Renders (overwrites) `l` and `r`.
    fn render(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams);
    fn panel(p: Arc<FxParams>) -> Arc<dyn Extension>;
}

pub struct Instrument<T: Synth> {
    params: Arc<FxParams>,
    dsp: T,
}
impl<T: Synth> Default for Instrument<T> {
    fn default() -> Self {
        Self {
            params: Arc::new(FxParams::new(T::SPEC)),
            dsp: T::default(),
        }
    }
}
impl<T: Synth> Plugin for Instrument<T> {
    const NAME: &'static str = T::SPEC.name;
    const VENDOR: &'static str = "Rind Studio";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let mut shared = Shared::default();
        shared.extension = Some(T::panel(self.params.clone()));
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
        _: &mut impl InitContext<Self>,
    ) -> bool {
        if !(8000.0..=192000.0).contains(&c.sample_rate) {
            return false;
        }
        self.dsp.prepare(c.sample_rate, &self.params);
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
        let c = b.as_slice();
        if c.len() < 2 {
            return ProcessStatus::KeepAlive;
        }
        let (l, r) = c.split_at_mut(1);
        let (l, r) = (&mut *l[0], &mut *r[0]);
        let n = l.len();
        if let Some(bpm) = ctx.transport().tempo {
            self.dsp.tempo(bpm);
        }
        let mut at = 0;
        while let Some(event) = ctx.next_event() {
            let t = (event.timing() as usize).min(n);
            if t > at {
                self.dsp.render(&mut l[at..t], &mut r[at..t], &self.params);
                at = t;
            }
            match event {
                NoteEvent::NoteOn {
                    voice_id,
                    channel,
                    note,
                    velocity,
                    ..
                } => self.dsp.note_on(channel, note, velocity, voice_id),
                NoteEvent::NoteOff {
                    voice_id,
                    channel,
                    note,
                    ..
                } => self.dsp.note_off(channel, note, voice_id),
                NoteEvent::MidiPitchBend { value, .. } => self.dsp.pitch_bend(value),
                NoteEvent::MidiCC { cc, value, .. } => self.dsp.control(cc, value),
                NoteEvent::MidiChannelPressure { pressure, .. } => self.dsp.pressure(pressure),
                _ => {}
            }
        }
        if at < n {
            self.dsp.render(&mut l[at..], &mut r[at..], &self.params);
        }
        ProcessStatus::KeepAlive
    }
}
impl<T: Synth> ClapPlugin for Instrument<T> {
    const CLAP_ID: &'static str = T::ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(T::SPEC.about);
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}
impl<T: Synth> Vst3Plugin for Instrument<T> {
    const VST3_CLASS_ID: [u8; 16] = T::CLASS;
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}
