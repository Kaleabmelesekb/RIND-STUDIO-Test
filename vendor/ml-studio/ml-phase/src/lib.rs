// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, semicolon_in_expressions_from_non_local_macros)]
use ml_small_fx::{
    Card, Control, Mark, Setting, Shared, Spec,
    editor::{Extension, FxEditor},
    params::FxParams,
};
use nih_plug::prelude::*;
use os_ui::{Canvas, Color, Font, Rect};
use std::{
    num::NonZeroU32,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
mod analysis;
mod dsp;
use dsp::{
    delay::{AlignDelay, TapSpec},
    fractional::FIR_CENTER,
};
const CAPACITY: usize = 384000;
struct Capture {
    main: Vec<AtomicU32>,
    reference: Vec<AtomicU32>,
    state: AtomicU64,
    rate: AtomicU32,
    count: AtomicU32,
    result: Mutex<Option<(u64, f64, analysis::AnalysisResult)>>,
    message: Mutex<(u64, String)>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
}
impl Capture {
    fn cancel(&self) {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| Some((v & !7) + 8))
            .ok();
    }
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            main: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            reference: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            state: AtomicU64::new(0),
            rate: AtomicU32::new(48000),
            count: AtomicU32::new(0),
            workers: Mutex::new(Vec::new()),
            result: Mutex::new(None),
            message: Mutex::new((
                0,
                "Route reference to sidechain, play both tracks, then Capture.".into(),
            )),
        }
    }
}
struct Panel {
    params: Arc<FxParams>,
    capture: Arc<Capture>,
}
impl Panel {
    fn start(&self) {
        let c = self.capture.clone();
        if matches!(c.state.load(Ordering::Acquire) & 7, 1..=4) {
            c.cancel();
            *c.message.lock().unwrap() = (
                c.state.load(Ordering::Acquire) & !7,
                "Capture cancelled.".into(),
            );
            return;
        }
        let token = (c.state.load(Ordering::Acquire) & !7) + 8;
        *c.result.lock().unwrap() = None;
        *c.message.lock().unwrap() = (token, "Capturing 2 seconds of playback...".into());
        c.state.store(token | 1, Ordering::Release);
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while c.state.load(Ordering::Acquire) != (token | 3) {
                if c.state.load(Ordering::Acquire) & !7 != token {
                    return;
                }
                if Instant::now() > deadline {
                    *c.message.lock().unwrap() = (
                        token,
                        "No complete capture. Start playback and check the sidechain.".into(),
                    );
                    let _ = c.state.compare_exchange(
                        token | 2,
                        token,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    let _ = c.state.compare_exchange(
                        token | 1,
                        token,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if c.state.load(Ordering::Acquire) & !7 != token {
                return;
            }
            if c.state
                .compare_exchange(token | 3, token | 4, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            let n = c.count.load(Ordering::Acquire) as usize;
            let a: Vec<_> = c.main[..n]
                .iter()
                .map(|x| f32::from_bits(x.load(Ordering::Relaxed)))
                .collect();
            let b: Vec<_> = c.reference[..n]
                .iter()
                .map(|x| f32::from_bits(x.load(Ordering::Relaxed)))
                .collect();
            let rate = c.rate.load(Ordering::Acquire) as f64;
            let report = analysis::analyze_detailed(&a, &b, (rate * 0.05).floor() as usize);
            if c.state.load(Ordering::Acquire) & !7 != token {
                return;
            }
            let mut message = c.message.lock().unwrap();
            match report.outcome {
                Ok(result) => {
                    *message = (
                        token,
                        format!(
                            "Suggested: {:+.2} ms, {} polarity, confidence {:.0}%",
                            result.offset_samples / rate * 1000.0,
                            if result.inverted { "reverse" } else { "normal" },
                            result.confidence * 100.0
                        ),
                    );
                    *c.result.lock().unwrap() = Some((token, rate, result));
                    let _ = c.state.compare_exchange(
                        token | 4,
                        token | 5,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                }
                Err(reason) => {
                    *message = (
                        token,
                        format!(
                            "{}",
                            match reason {
                                analysis::RejectReason::Silence =>
                                    "No reference signal. Check playback and sidechain routing.",
                                analysis::RejectReason::LowConfidence =>
                                    "These signals do not match closely enough. Try another passage.",
                                analysis::RejectReason::TooShort =>
                                    "Not enough audio. Capture a longer passage.",
                                analysis::RejectReason::NonFinite =>
                                    "Invalid audio from an upstream effect. Check the signal chain.",
                            }
                        ),
                    );
                    let _ = c.state.compare_exchange(
                        token | 4,
                        token,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                }
            }
        });
        let mut workers = self.capture.workers.lock().unwrap();
        workers.retain(|w| !w.is_finished());
        workers.push(worker);
    }
}
impl Extension for Panel {
    fn editor_closed(&self) {
        self.capture.cancel();
    }
    fn paint(&self, c: &mut Canvas, model: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        let busy = matches!(self.capture.state.load(Ordering::Acquire) & 7, 1..=4);
        for (x, label) in [
            (16., if busy { "Cancel" } else { "Capture" }),
            (140., "Apply suggestion"),
        ] {
            os_ui::kit::inst::text_button(
                c,
                Rect::new(x, 156., 116., 28.),
                label,
                os_ui::kit::State::NORMAL.with_disabled(
                    x == 140. && self.capture.state.load(Ordering::Acquire) & 7 != 5,
                ),
            );
        }
        c.text(
            Font::new(12),
            52.,
            126.,
            Color::rgb(0xdddddd),
            &model.text("offset"),
        );
        if let Ok((token, msg)) = self.capture.message.try_lock().as_deref() {
            let state = self.capture.state.load(Ordering::Acquire);
            let status = if state & !7 != *token {
                "Ready. Play both tracks, then Capture again."
            } else if state & 7 == 4 {
                "Analyzing captured audio..."
            } else {
                msg
            };
            c.text_clip(Font::new(11), 16., 200., 568., Color::rgb(0xdddddd), status);
        }
        c.text(
            Font::new(11),
            16.,
            225.,
            Color::rgb(0xaaaaaa),
            "Measures left channels. Applies the same correction to both channels.",
        );
    }
    fn click(&self, x: f32, y: f32, ctx: Option<&Arc<dyn GuiContext>>) -> bool {
        if !os_plugkit::visibility::stock_dsp_controls() || !(156.0..184.0).contains(&y) {
            return false;
        }
        if (16.0..132.0).contains(&x) {
            self.start();
            return true;
        }
        if (140.0..256.0).contains(&x) {
            if let (Some(ctx), Some((token, rate, result))) =
                (ctx, *self.capture.result.lock().unwrap())
            {
                if self.capture.state.load(Ordering::Acquire) != (token | 5) {
                    return true;
                }
                let setter = ParamSetter::new(ctx.as_ref());
                let shift = (result.offset_samples / rate * 1000000.0).round() as i32;
                for (index, value) in [
                    (0, shift.clamp(-50000, 50000)),
                    (1, i32::from(result.inverted)),
                ] {
                    setter.begin_set_parameter(&self.params.p[index]);
                    setter.set_parameter(&self.params.p[index], value);
                    setter.end_set_parameter(&self.params.p[index]);
                }
            }
            return true;
        }
        false
    }
}
fn text(k: usize, v: i32) -> String {
    if k == 0 {
        format!("{:+.3} ms", v as f64 / 1000.)
    } else {
        if v == 0 { "Normal" } else { "Reverse" }.into()
    }
}
fn parse(k: usize, s: &str) -> Option<i32> {
    if k == 0 {
        let v = s.trim().trim_end_matches("ms").trim().parse::<f64>().ok()?;
        v.is_finite()
            .then(|| ((v * 1000.).round() as i32).clamp(-50000, 50000))
    } else {
        match s.trim().to_lowercase().as_str() {
            "normal" => Some(0),
            "reverse" => Some(1),
            _ => None,
        }
    }
}
pub static SPEC: Spec = Spec {
    name: "ML Phase",
    word: "PHASE",
    about: "ML Phase: time and polarity alignment. DSP derived from ConjureAlign (GPL-3.0-or-later).",
    settings: &[
        Setting {
            id: "offset",
            name: "Timing offset",
            range: (-50000, 50000),
            start: 0,
            magnet: Some(0),
        },
        Setting {
            id: "polarity",
            name: "Polarity",
            range: (0, 1),
            start: 0,
            magnet: None,
        },
    ],
    studio_settings: 2,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (600, 280),
    cards: &[
        Card {
            x0: 8.,
            x1: 170.,
            y0: 28.,
            y1: 142.,
            heading: "TIMING",
        },
        Card {
            x0: 178.,
            x1: 340.,
            y0: 28.,
            y1: 142.,
            heading: "POLARITY",
        },
    ],
    controls: &[
        Control::Knob {
            id: "offset",
            cx: 89.,
            cy: 77.,
            r: 26.,
            caption: "Timing",
            bipolar: true,
        },
        Control::Choice {
            id: "polarity",
            x: 198.,
            top: 62.,
            names: &["Normal", "Reverse"],
        },
    ],
    display: None,
    mark: Mark {
        x: 584.,
        y: 262.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[
        (470., 60., "REFERENCE VIA SIDECHAIN"),
        (470., 84., "Capture > review > apply"),
        (470., 110., "Mixing / ~50 ms lookahead"),
    ],
    text,
    parse,
    sound: |_, _| unreachable!("ML Phase uses its own sidechain processor"),
    rings: true,
};
pub struct MlPhase {
    params: Arc<FxParams>,
    capture: Arc<Capture>,
    delay: Option<AlignDelay>,
    base: f64,
    count: usize,
}
impl Default for MlPhase {
    fn default() -> Self {
        Self {
            params: Arc::new(FxParams::new(&SPEC)),
            capture: Arc::default(),
            delay: None,
            base: 0.,
            count: 0,
        }
    }
}
impl Drop for MlPhase {
    fn drop(&mut self) {
        self.capture.cancel();
        if let Ok(mut workers) = self.capture.workers.lock() {
            for worker in workers.drain(..) {
                let _ = worker.join();
            }
        }
    }
}
impl Plugin for MlPhase {
    const NAME: &'static str = "ML Phase";
    const VENDOR: &'static str = "ML Studio";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[new_nonzero_u32(2)],
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            aux_input_ports: &[new_nonzero_u32(1)],
            ..AudioIOLayout::const_default()
        },
    ];
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let mut shared = Shared::default();
        shared.extension = Some(Arc::new(Panel {
            params: self.params.clone(),
            capture: self.capture.clone(),
        }));
        Some(Box::new(FxEditor::new(
            &SPEC,
            self.params.clone(),
            Arc::new(shared),
        )))
    }
    fn initialize(
        &mut self,
        layout: &AudioIOLayout,
        c: &BufferConfig,
        ctx: &mut impl InitContext<Self>,
    ) -> bool {
        if !(8000.0..=192000.0).contains(&c.sample_rate) {
            return false;
        }
        self.capture.cancel();
        self.capture
            .rate
            .store(c.sample_rate.round() as u32, Ordering::Release);
        self.base = (c.sample_rate as f64 * 0.05).ceil() + FIR_CENTER as f64;
        self.delay = Some(AlignDelay::new(
            layout.main_output_channels.unwrap().get() as usize,
            (self.base + c.sample_rate as f64 * 0.05).ceil() as usize,
            (c.sample_rate * 0.01) as usize,
        ));
        ctx.set_latency_samples(self.base as u32);
        self.reset();
        true
    }
    fn reset(&mut self) {
        self.capture.cancel();
        self.count = 0;
        if let Some(d) = self.delay.as_mut() {
            d.retarget(TapSpec {
                delay_samples: self.base
                    + self.params.v(0) as f64 / 1000000.0
                        * self.capture.rate.load(Ordering::Relaxed) as f64,
                inverted: self.params.v(1) != 0,
            });
            d.reset();
        }
    }
    fn process(
        &mut self,
        b: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        _: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let c = &self.capture;
        let request = c.state.load(Ordering::Acquire);
        if request & 7 == 1
            && c.state
                .compare_exchange(
                    request,
                    (request & !7) | 2,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
        {
            self.count = 0;
        }
        let active = c.state.load(Ordering::Acquire);
        if active & 7 == 2 {
            let input = b.as_slice();
            let reference = aux.inputs.first_mut().map(|x| x.as_slice());
            let target = (c.rate.load(Ordering::Relaxed) as usize * 2).min(CAPACITY);
            for i in 0..input.first().map_or(0, |x| x.len()) {
                if self.count == target {
                    break;
                }
                let main = input[0][i];
                let reference = reference
                    .as_ref()
                    .and_then(|x| x.first())
                    .and_then(|x| x.get(i))
                    .copied()
                    .unwrap_or(0.);
                c.main[self.count].store(main.to_bits(), Ordering::Relaxed);
                c.reference[self.count].store(reference.to_bits(), Ordering::Relaxed);
                self.count += 1;
            }
            if self.count == target {
                c.count.store(target as u32, Ordering::Release);
                let _ = c.state.compare_exchange(
                    active,
                    (active & !7) | 3,
                    Ordering::Release,
                    Ordering::Relaxed,
                );
            }
        }
        if let Some(d) = self.delay.as_mut() {
            d.retarget(TapSpec {
                delay_samples: self.base
                    + self.params.v(0).clamp(-50000, 50000) as f64 / 1000000.0
                        * c.rate.load(Ordering::Relaxed) as f64,
                inverted: self.params.v(1) != 0,
            });
            d.process(b.as_slice());
        }
        ProcessStatus::KeepAlive
    }
}
impl ClapPlugin for MlPhase {
    const CLAP_ID: &'static str = "com.mlstudio.phase";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Time and polarity alignment with reference sidechain");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
    ];
}
impl Vst3Plugin for MlPhase {
    const VST3_CLASS_ID: [u8; 16] = *b"MLStudioPhase001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
#[cfg(not(feature = "linked"))]
nih_export_clap!(MlPhase);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(MlPhase);
os_plugkit::linked_clap_entry!(MlPhase);

#[cfg(test)]
mod workflow_tests {
    use super::*;
    #[test]
    fn cancellation_invalidates_old_completion() {
        let c = Capture::default();
        c.state.store(10, Ordering::Release);
        c.cancel();
        assert!(
            c.state
                .compare_exchange(10, 11, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        );
        assert_eq!(c.state.load(Ordering::Acquire) & 7, 0);
    }
    #[test]
    fn timing_text_and_state_roundtrip() {
        for v in [-5000, -23, 0, 431, 5000] {
            assert_eq!(parse(0, &text(0, v)), Some(v));
        }
        assert_eq!(parse(0, "NaN"), None);
        let a = FxParams::new(&SPEC);
        let b = FxParams::new(&SPEC);
        b.deserialize_fields(&a.serialize_fields());
        assert_eq!(a.raw_copy(), b.raw_copy());
    }
    #[test]
    fn correlated_tap_changes_never_raise_dc_gain() {
        let mut d = AlignDelay::new(1, 1000, 100);
        d.retarget(TapSpec {
            delay_samples: 100.,
            inverted: false,
        });
        d.reset();
        let mut warm = vec![0.25; 2000];
        d.process(&mut [&mut warm]);
        d.retarget(TapSpec {
            delay_samples: 101.,
            inverted: false,
        });
        let mut out = vec![0.25; 500];
        d.process(&mut [&mut out]);
        assert!(out.iter().all(|x| (*x - 0.25).abs() < 0.00001));
    }
    #[test]
    fn captured_reference_produces_suggestion_without_changing_controls() {
        let p = MlPhase::default();
        let panel = Panel {
            params: p.params.clone(),
            capture: p.capture.clone(),
        };
        panel.start();
        let token = p.capture.state.load(Ordering::Acquire) & !7;
        let n = 96000;
        let mut seed = 17u32;
        for i in 0..n {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let sample = (seed as i32 as f32 / i32::MAX as f32) * 0.2;
            p.capture.main[i].store(sample.to_bits(), Ordering::Relaxed);
            if i + 240 < n {
                p.capture.reference[i + 240].store((-sample).to_bits(), Ordering::Relaxed);
            }
        }
        p.capture.count.store(n as u32, Ordering::Release);
        p.capture.state.store(token | 3, Ordering::Release);
        for worker in p.capture.workers.lock().unwrap().drain(..) {
            worker.join().unwrap();
        }
        assert_eq!(p.capture.state.load(Ordering::Acquire), token | 5);
        let result = p.capture.result.lock().unwrap().unwrap().2;
        assert!((result.offset_samples - 240.).abs() < 0.1);
        assert!(result.inverted);
        assert!(result.confidence > 0.99);
        assert_eq!(p.params.v(0), 0);
        assert_eq!(p.params.v(1), 0);
        panel.editor_closed();
        assert_eq!(p.capture.state.load(Ordering::Acquire) & 7, 0);
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;
    use os_engine::HostedEffect as _;
    use os_host::Plugin as _;
    #[test]
    fn native_host_pdc_processing_and_state() {
        let entry = &LINKED_CLAP_ENTRY as *const _ as *const std::ffi::c_void;
        let lib =
            unsafe { os_clap::Library::from_entry(entry, std::path::Path::new("ML Phase.clap")) }
                .unwrap();
        for rate in [44100, 48000, 96000, 192000] {
            let mut p = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
            assert!(p.activate(rate));
            let baseline = p.latency();
            assert_eq!(
                baseline,
                (rate as f64 * 0.05).ceil() as u32 + FIR_CENTER as u32
            );
            let offset = (0..p.param_count())
                .filter_map(|i| p.param_info(i))
                .find(|x| x.name == "Timing offset")
                .unwrap()
                .id;
            let polarity = (0..p.param_count())
                .filter_map(|i| p.param_info(i))
                .find(|x| x.name == "Polarity")
                .unwrap()
                .id;
            p.set_param(offset, 0.6);
            p.set_param(polarity, 1.0);
            let mut l = vec![0.; rate as usize / 4];
            let mut rr = l.clone();
            p.process_fx(rate, &mut l, &mut rr);
            p.idle();
            let saved = p.save_state().unwrap();
            let mut restored = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
            assert!(restored.load_state(&saved));
            assert!(restored.activate(rate));
            assert_eq!(restored.latency(), baseline);
            assert!((restored.param_value(offset).unwrap() - 0.6).abs() < 0.00001);
            l.fill(0.);
            rr.fill(0.);
            l[0] = 1.;
            rr[0] = 0.5;
            restored.process_fx(rate, &mut l, &mut rr);
            let peak = l
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
                .unwrap()
                .0;
            assert_eq!(
                peak,
                baseline as usize + (rate as f64 * 0.01).round() as usize
            );
            assert!((l[peak] + 1.).abs() < 0.001);
            assert!((rr[peak] + 0.5).abs() < 0.001);
        }
    }
    #[test]
    fn stock_editor_render() {
        struct Model;
        impl ml_small_fx::view::Model for Model {
            fn norm(&self, id: &str) -> f32 {
                if id == "offset" { 0.5 } else { 0. }
            }
            fn plain(&self, _: &str) -> f32 {
                0.
            }
            fn text(&self, id: &str) -> String {
                text(if id == "offset" { 0 } else { 1 }, 0)
            }
        }
        if let Ok(dir) = std::env::var("ML_PHASE_RENDER_DIR") {
            let panel = Panel {
                params: Arc::new(FxParams::new(&SPEC)),
                capture: Arc::default(),
            };
            for scale in [1., 1.5, 2.] {
                let mut canvas =
                    Canvas::new((600. * scale) as u32, (280. * scale) as u32, scale).unwrap();
                let hits = ml_small_fx::view::paint(
                    &SPEC,
                    &mut canvas,
                    &os_theme::Palette::default(),
                    &Model,
                    &Default::default(),
                    "",
                );
                assert_eq!(hits.iter().filter(|h| h.t.id() == "offset").count(), 1);
                panel.paint(&mut canvas, &Model);
                canvas
                    .pixmap
                    .save_png(std::path::Path::new(&dir).join(format!("ML Phase {scale}.png")))
                    .unwrap();
            }
        }
    }
}
