use crate::*;
use ebur128::{EbuR128, Mode};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
struct Track {
    sample: Option<Arc<os_engine::sample::Sample>>,
    peak: f32,
    revision: u64,
}
#[cfg(test)]
thread_local! {pub(crate) static TEST_SHARED: std::cell::RefCell<Option<Arc<SharedRef>>> = const { std::cell::RefCell::new(None) };}
pub(crate) struct SharedRef {
    path: Mutex<String>,
    revision: AtomicU64,
    pending: AtomicPtr<Track>,
    retired: AtomicPtr<Track>,
    stop: AtomicBool,
    pub listen: AtomicBool,
    message: Mutex<String>,
    edits: Mutex<Vec<String>>,
    levels: [AtomicU32; 3],
    ready: AtomicBool,
    invalid_range: AtomicBool,
}
impl SharedRef {
    fn new() -> Self {
        Self {
            path: Mutex::new(String::new()),
            revision: AtomicU64::new(1),
            pending: AtomicPtr::new(std::ptr::null_mut()),
            retired: AtomicPtr::new(std::ptr::null_mut()),
            stop: AtomicBool::new(false),
            listen: AtomicBool::new(false),
            message: Mutex::new("Load a song, play your mix, then compare.".into()),
            edits: Mutex::new(Vec::new()),
            levels: std::array::from_fn(|_| AtomicU32::new(f32::NEG_INFINITY.to_bits())),
            ready: AtomicBool::new(false),
            invalid_range: AtomicBool::new(false),
        }
    }
    pub(crate) fn request(&self, path: String, undo: bool) {
        let mut old = self.path.lock().unwrap();
        if undo {
            let patch=serde_json::json!({"fields":{"reference_path":serde_json::to_string(&*old).unwrap()}}).to_string();
            let mut q = self.edits.lock().unwrap();
            if q.len() >= 128 {
                q.remove(0);
            }
            q.push(patch);
        }
        *old = path;
        self.listen.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
        self.revision.fetch_add(1, Ordering::AcqRel);
        *self.message.lock().unwrap() = "Loading reference…".into();
    }
    fn reclaim(slot: &AtomicPtr<Track>) {
        let p = slot.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if !p.is_null() {
            unsafe {
                drop(Box::from_raw(p));
            }
        }
    }
}
impl Drop for SharedRef {
    fn drop(&mut self) {
        Self::reclaim(&self.pending);
        Self::reclaim(&self.retired);
    }
}
pub(crate) struct RefParams {
    pub fx: Arc<FxParams>,
    pub shared: Arc<SharedRef>,
}
unsafe impl Params for RefParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        self.fx.param_map()
    }
    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut f = self.fx.serialize_fields();
        f.insert(
            "reference_path".into(),
            serde_json::to_string(&*self.shared.path.lock().unwrap()).unwrap(),
        );
        f
    }
    fn deserialize_fields(&self, f: &BTreeMap<String, String>) {
        self.fx.deserialize_fields(f);
        if let Some(path) = f
            .get("reference_path")
            .and_then(|s| serde_json::from_str::<String>(s).ok())
            .filter(|s| s.len() < 32768)
        {
            self.shared.request(path, false);
        }
    }
}
pub(crate) struct Panel {
    pub shared: Arc<SharedRef>,
}
impl Extension for Panel {
    fn take_state_edits(&self) -> Vec<String> {
        std::mem::take(&mut *self.shared.edits.lock().unwrap())
    }
    fn accepts_files(&self) -> bool {
        true
    }
    fn file_dropped(&self, path: &Path) {
        self.shared
            .request(path.to_string_lossy().into_owned(), true);
    }
    fn paint(&self, c: &mut Canvas, _: &dyn ml_small_fx::view::Model) {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return;
        }
        button(c, Rect::new(16., 168., 140., 29.), "Load / relink…", false);
        button(c, Rect::new(166., 168., 90., 29.), "Clear", false);
        let path = self.shared.path.lock().unwrap();
        let name = Path::new(&*path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        text(c, 270., 175., &name.chars().take(48).collect::<String>());
        drop(path);
        let listen = self.shared.listen.load(Ordering::Acquire);
        button(c, Rect::new(16., 212., 150., 34.), "Your mix", !listen);
        button(c, Rect::new(176., 212., 150., 34.), "Reference", listen);
        text(c, 346., 222., "A/B follows the DAW transport");
        for (i, label) in ["YOUR MIX", "REFERENCE RAW", "MATCH GAIN"]
            .iter()
            .enumerate()
        {
            let x = 16. + i as f32 * 210.;
            text(c, x, 266., label);
            let value = f32::from_bits(self.shared.levels[i].load(Ordering::Relaxed));
            text(
                c,
                x,
                286.,
                &if value.is_finite() {
                    format!("{value:.1} {}", if i < 2 { "LUFS" } else { "dB" })
                } else {
                    "Measuring…".into()
                },
            );
        }
        let status = if self.shared.invalid_range.load(Ordering::Relaxed) {
            "Start is past the end of this reference. Reduce Start.".into()
        } else if listen
            && !self.shared.ready.load(Ordering::Relaxed)
            && self.shared.message.lock().unwrap().starts_with("Ready.")
        {
            "Play your mix for at least 0.4 s to establish the level match.".into()
        } else {
            self.shared.message.lock().unwrap().clone()
        };
        text(c, 16., 322., &status.chars().take(90).collect::<String>());
        text(
            c,
            16.,
            346.,
            "Use last on Master • loudness match freezes while listening to Reference",
        );
        text(
            c,
            16.,
            369.,
            "Monitoring only: offline exports always keep your mix. Files stay linked.",
        );
    }
    fn click(&self, x: f32, y: f32, _: Option<&Arc<dyn GuiContext>>) -> bool {
        if !os_plugkit::visibility::stock_dsp_controls() {
            return false;
        }
        if Rect::new(16., 168., 140., 29.).contains(x, y) {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Load or relink reference song")
                .add_filter("Audio", &["wav", "flac", "mp3", "ogg", "aif", "aiff"])
                .pick_file()
            {
                self.file_dropped(&path);
            }
            return true;
        }
        if Rect::new(166., 168., 90., 29.).contains(x, y) {
            self.shared.request(String::new(), true);
            return true;
        }
        if Rect::new(16., 212., 150., 34.).contains(x, y) {
            self.shared.listen.store(false, Ordering::Release);
            return true;
        }
        if Rect::new(176., 212., 150., 34.).contains(x, y) {
            self.shared.listen.store(true, Ordering::Release);
            return true;
        }
        false
    }
}
pub struct Reference {
    pub(crate) params: Arc<RefParams>,
    worker: Option<JoinHandle<()>>,
    track: Option<Box<Track>>,
    meters: Option<[EbuR128; 2]>,
    rate: u32,
    pub(crate) offline: bool,
    blend: f32,
    gain: f32,
    target: f32,
    frames: u64,
    tick: usize,
    last_end: Option<i64>,
    fallback: i64,
    last_region: [i32; 2],
}
impl Default for Reference {
    fn default() -> Self {
        let shared = Arc::new(SharedRef::new());
        #[cfg(test)]
        TEST_SHARED.with(|s| *s.borrow_mut() = Some(shared.clone()));
        let params = Arc::new(RefParams {
            fx: Arc::new(FxParams::new(&specs::REFERENCE)),
            shared: shared.clone(),
        });
        let worker = thread::spawn(move || {
            let mut seen = 0;
            while !shared.stop.load(Ordering::Acquire) {
                SharedRef::reclaim(&shared.retired);
                let revision = shared.revision.load(Ordering::Acquire);
                if revision != seen {
                    let path = shared.path.lock().unwrap().clone();
                    let result =
                        (|| -> Result<(Option<Arc<os_engine::sample::Sample>>, f32), String> {
                            if path.is_empty() {
                                return Ok((None, 0.));
                            }
                            let meta = std::fs::metadata(&path).map_err(|_| {
                                "Missing file. Use Load / relink to locate the song.".to_string()
                            })?;
                            if meta.len() > 256 * 1024 * 1024 {
                                return Err("Reference file exceeds 256 MiB.".into());
                            }
                            let sample = os_engine::sample::load_shared(Path::new(&path))
                                .map_err(|e| format!("Could not load reference: {e}"))?;
                            if !(8000..=192000).contains(&sample.rate)
                                || sample.frames() == 0
                                || sample.frames() > sample.rate as usize * 1200
                            {
                                return Err("Use audio between 8–192 kHz, up to 20 minutes.".into());
                            }
                            let mut peak = 0f32;
                            for x in sample.left.iter().chain(&sample.right) {
                                if !x.is_finite() {
                                    return Err("Reference contains invalid samples.".into());
                                }
                                peak = peak.max(x.abs());
                            }
                            Ok((Some(sample), peak))
                        })();
                    if shared.revision.load(Ordering::Acquire) == revision {
                        let (data, peak, msg) = match result {
                            Ok((s, p)) => {
                                let msg = if s.is_some() {
                                    "Ready. Reference gain is capped using source peaks (-1 dBFS target)."
                                } else {
                                    "Load a song, play your mix, then compare."
                                };
                                (s, p, msg.to_string())
                            }
                            Err(e) => (None, 0., e),
                        };
                        let old = shared.pending.swap(
                            Box::into_raw(Box::new(Track {
                                sample: data,
                                peak,
                                revision,
                            })),
                            Ordering::AcqRel,
                        );
                        if !old.is_null() {
                            unsafe {
                                drop(Box::from_raw(old));
                            }
                        }
                        *shared.message.lock().unwrap() = msg;
                    }
                    seen = revision;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            params,
            worker: Some(worker),
            track: None,
            meters: None,
            rate: 48000,
            offline: false,
            blend: 0.,
            gain: 1.,
            target: 1.,
            frames: 0,
            tick: 0,
            last_end: None,
            fallback: 0,
            last_region: [0, 0],
        }
    }
}
impl Drop for Reference {
    fn drop(&mut self) {
        self.params.shared.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
impl Reference {
    #[cfg(test)]
    pub(crate) fn loaded(&self) -> bool {
        self.track.as_ref().is_some_and(|t| {
            t.sample.is_some() && t.revision == self.params.shared.revision.load(Ordering::Acquire)
        })
    }
    #[cfg(test)]
    pub(crate) fn status(&self) -> String {
        self.params.shared.message.lock().unwrap().clone()
    }
    pub(crate) fn prepare(&mut self, rate: u32) {
        self.rate = rate;
        self.meters = Some(std::array::from_fn(|_| {
            EbuR128::new(2, rate, Mode::M | Mode::S).unwrap()
        }));
        os_dsp::resample::weights(
            0.3,
            1.1,
            16,
            os_dsp::resample::Quality::Table,
            &mut [0.; 32],
        );
        self.reset();
    }
    fn reset_measure(&mut self) {
        if let Some(m) = &mut self.meters {
            for e in m {
                e.reset();
            }
        }
        self.frames = 0;
        self.tick = 0;
        self.params.shared.ready.store(false, Ordering::Release);
    }
    pub(crate) fn audio(&mut self, l: &mut [f32], r: &mut [f32], playing: bool, pos: Option<i64>) {
        if self.offline {
            return;
        }
        if self.blend < 1e-6 && self.params.shared.retired.load(Ordering::Acquire).is_null() {
            let ptr = self
                .params
                .shared
                .pending
                .swap(std::ptr::null_mut(), Ordering::AcqRel);
            if !ptr.is_null() {
                let next = unsafe { Box::from_raw(ptr) };
                if let Some(old) = self.track.replace(next) {
                    self.params
                        .shared
                        .retired
                        .store(Box::into_raw(old), Ordering::Release);
                }
                self.reset_measure();
            }
        }
        let region = [self.params.fx.v(0), self.params.fx.v(1)];
        let pos = pos.unwrap_or(self.fallback);
        if self.last_end.is_some_and(|p| p != pos) || region != self.last_region {
            self.reset_measure();
            self.blend = 0.;
        }
        self.last_region = region;
        self.last_end = Some(if playing { pos + l.len() as i64 } else { pos });
        self.fallback = self.last_end.unwrap();
        let valid = self.track.as_ref().and_then(|t| t.sample.as_ref());
        // Retain the old audio through its fade when a file is changed or cleared.
        // Only the current generation may begin a new reference audition.
        let current = self
            .track
            .as_ref()
            .is_some_and(|t| t.revision == self.params.shared.revision.load(Ordering::Acquire));
        let mut range = None;
        if let Some(s) = valid {
            let start = region[0] as f64 / 10. * s.rate as f64;
            let length = (region[1] as f64 / 10. * s.rate as f64).min(s.frames() as f64 - start);
            if length > 1. {
                range = Some((start, length));
            }
        }
        self.params
            .shared
            .invalid_range
            .store(valid.is_some() && range.is_none(), Ordering::Relaxed);
        let match_on = self.params.fx.v(3) != 0;
        let listen = self.params.shared.listen.load(Ordering::Acquire);
        let trim = 10f32.powf(self.params.fx.v(2) as f32 / 200.);
        let limit = self
            .track
            .as_ref()
            .map_or(1., |t| if t.peak > 0. { 0.8912509 / t.peak } else { 1. });
        let smooth = 1. - (-1. / (0.2 * self.rate as f32)).exp();
        let step = 1. / (0.02 * self.rate as f32);
        let mut input = [0.; 512];
        let mut reference = [0.; 512];
        for start_index in (0..l.len()).step_by(256) {
            let n = (l.len() - start_index).min(256);
            for i in 0..n {
                let index = start_index + i;
                input[2 * i] = l[index];
                input[2 * i + 1] = r[index];
                let (mut a, mut b) = (0., 0.);
                if playing {
                    if let (Some(s), Some((start, length))) = (valid, range) {
                        let ratio = s.rate as f64 / self.rate as f64;
                        let within =
                            ((pos.max(0) as f64 + index as f64) * ratio).rem_euclid(length);
                        let sample = os_dsp::resample::read(
                            &s.left,
                            &s.right,
                            start + within,
                            ratio,
                            os_dsp::resample::Quality::Table,
                        );
                        let fade = (within / (s.rate as f64 * 0.003))
                            .min((length - within) / (s.rate as f64 * 0.003))
                            .clamp(0., 1.) as f32;
                        a = sample.0 * fade;
                        b = sample.1 * fade;
                    }
                }
                reference[2 * i] = a;
                reference[2 * i + 1] = b;
                let want = playing
                    && current
                    && range.is_some()
                    && listen
                    && (!match_on || self.params.shared.ready.load(Ordering::Relaxed));
                self.blend += (if want { 1. - self.blend } else { -self.blend }).clamp(-step, step);
                let target = (if match_on { self.target } else { 1. }) * trim;
                self.gain += smooth * (target.min(limit) - self.gain);
                let gain = self.gain.min(limit);
                if self.blend > 0. {
                    l[index] = input[2 * i] + (a * gain - input[2 * i]) * self.blend;
                    r[index] = input[2 * i + 1] + (b * gain - input[2 * i + 1]) * self.blend;
                }
            }
            if playing && range.is_some() {
                if let Some(m) = &mut self.meters {
                    let _ = m[0].add_frames_f32(&input[..n * 2]);
                    let _ = m[1].add_frames_f32(&reference[..n * 2]);
                    self.frames += n as u64;
                    self.tick += n;
                    if self.tick >= self.rate as usize / 10 {
                        self.tick = 0;
                        let get = |m: &EbuR128| {
                            if self.frames >= self.rate as u64 * 3 {
                                m.loudness_shortterm()
                            } else {
                                m.loudness_momentary()
                            }
                        };
                        let a = get(&m[0]).unwrap_or(f64::NEG_INFINITY);
                        let b = get(&m[1]).unwrap_or(f64::NEG_INFINITY);
                        self.params.shared.levels[0].store((a as f32).to_bits(), Ordering::Relaxed);
                        self.params.shared.levels[1].store((b as f32).to_bits(), Ordering::Relaxed);
                        if self.frames >= self.rate as u64 * 4 / 10
                            && a.is_finite()
                            && b.is_finite()
                            && a > -60.
                            && b > -60.
                            && (!listen || !self.params.shared.ready.load(Ordering::Acquire))
                        {
                            self.target = 10f32.powf(((a - b) as f32).clamp(-18., 18.) / 20.);
                            if !self.params.shared.ready.swap(true, Ordering::AcqRel) {
                                self.gain = (self.target * trim).min(limit);
                            }
                        }
                        self.params.shared.levels[2].store(
                            (20. * self.gain.max(1e-9).log10()).to_bits(),
                            Ordering::Relaxed,
                        );
                    }
                }
            }
        }
    }
}
impl Plugin for Reference {
    const NAME: &'static str = "ML Reference";
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
        let mut s = Shared::default();
        s.extension = Some(Arc::new(Panel {
            shared: self.params.shared.clone(),
        }));
        Some(Box::new(FxEditor::new(
            &specs::REFERENCE,
            self.params.fx.clone(),
            Arc::new(s),
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
        self.offline = c.process_mode == ProcessMode::Offline;
        self.prepare(c.sample_rate.round() as u32);
        ctx.set_latency_samples(0);
        true
    }
    fn reset(&mut self) {
        self.reset_measure();
        self.blend = 0.;
        self.gain = 1.;
        self.target = 1.;
        self.last_end = None;
        self.fallback = 0;
    }
    fn process(
        &mut self,
        b: &mut Buffer,
        _: &mut AuxiliaryBuffers,
        ctx: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let t = ctx.transport();
        let playing = t.playing;
        let pos = t.pos_samples();
        let ch = b.as_slice();
        if ch.len() >= 2 {
            let (l, r) = ch.split_at_mut(1);
            self.audio(l[0], r[0], playing, pos);
        }
        ProcessStatus::KeepAlive
    }
}
impl ClapPlugin for Reference {
    const CLAP_ID: &'static str = "com.mlstudio.reference";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Level-matched reference monitoring; offline export passes the mix");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
    ];
}
impl Vst3Plugin for Reference {
    const VST3_CLASS_ID: [u8; 16] = *b"MLReference00001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}
