use super::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::Ordering,
};
struct Alloc;
thread_local! {static COUNT:Cell<Option<usize>>=const{Cell::new(None)};}
#[global_allocator]
static ALLOC: Alloc = Alloc;
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1))
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1))
            }
        });
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1))
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
}
fn no_alloc(f: impl FnOnce()) {
    COUNT.with(|c| c.set(Some(0)));
    f();
    let n = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(n, 0, "audio callback allocation/free");
}
fn set(p: &FxParams, i: usize, v: i32) {
    p.live.set(p.p[i].as_ptr(), p.p[i].preview_normalized(v));
}
fn tone(n: usize, rate: f64, f: f64, a: f32) -> Vec<f32> {
    (0..n)
        .map(|i| (std::f64::consts::TAU * f * i as f64 / rate).sin() as f32 * a)
        .collect()
}
fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}
#[test]
fn split_flat_sum_band_isolation_and_real_time() {
    for rate in [8000., 44100., 48000., 96000., 192000.] {
        for freq in [40., 250., 1000., rate * 0.4] {
            let input = tone(rate as usize, rate, freq, 0.2);
            let mut sum = vec![0.; input.len()];
            let mut all = Vec::new();
            for band in 0..4 {
                let p = FxParams::new(&specs::SPLIT);
                set(&p, 6, band);
                let mut s = split::Splitter::default();
                s.prepare(rate as f32, &p);
                let mut l = input.clone();
                let mut r = vec![0.; l.len()];
                no_alloc(|| s.audio(&mut l, &mut r, &p));
                assert_eq!(rms(&r), 0.);
                if band == 0 {
                    all = l;
                } else {
                    for (a, b) in sum.iter_mut().zip(l) {
                        *a += b;
                    }
                }
            }
            assert!(all.iter().zip(sum).all(|(a, b)| (a - b).abs() < 1e-6));
            let error = 20. * (rms(&all[all.len() / 2..]) / rms(&input[input.len() / 2..])).log10();
            assert!(error.abs() < 0.03, "rate {rate} freq {freq}: {error} dB");
        }
        let p = FxParams::new(&specs::SPLIT);
        set(&p, 6, 1);
        let mut s = split::Splitter::default();
        s.prepare(rate as f32, &p);
        let mut l = tone(8192, rate, rate * 0.35, 0.2);
        let mut r = l.clone();
        s.audio(&mut l, &mut r, &p);
        assert!(rms(&l[4096..]) < 0.003);
        for n in 0..20 {
            set(&p, 0, if n % 2 == 0 { 5000 } else { 20 });
            set(&p, 1, if n % 2 == 0 { 100 } else { 20000 });
            set(&p, 6, n % 4);
            let mut l = [0.1; 512];
            let mut r = l;
            no_alloc(|| s.audio(&mut l, &mut r, &p));
            assert!(l.iter().all(|x| x.is_finite() && x.abs() < 4.));
        }
        no_alloc(|| s.reset(&p));
        let mut l = [0.; 512];
        let mut r = l;
        no_alloc(|| s.audio(&mut l, &mut r, &p));
        assert_eq!(rms(&l), 0.);
    }
}
#[test]
fn shaper_delay_identity_and_alias_reduction() {
    let p = FxParams::new(&specs::SHAPER);
    set(&p, 0, 0);
    set(&p, 4, 0);
    let mut s = shaper::Shaper::default();
    s.prepare(48000., &p);
    let mut l = vec![0.; 256];
    l[0] = 0.2;
    let mut r = l.clone();
    no_alloc(|| s.audio(&mut l, &mut r, &p));
    assert_eq!(
        l.iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0,
        32
    );
    set(&p, 2, 0);
    s.reset(&p);
    l.fill(0.);
    l[0] = 0.2;
    r = l.clone();
    s.audio(&mut l, &mut r, &p);
    assert_eq!(l[32], 0.2);
    assert_eq!(l.iter().filter(|x| **x != 0.).count(), 1);
    set(&p, 2, 1000);
    s.reset(&p);
    let input = tone(48000, 48000., 1000., 0.2);
    l = input.clone();
    r = l.clone();
    s.audio(&mut l, &mut r, &p);
    assert!((rms(&l[2048..]) / rms(&input[2048..]) - 1.).abs() < 0.001);
    set(&p, 0, 180);
    s.reset(&p);
    let input = tone(96000, 48000., 7000., 0.5);
    l = input.clone();
    r = l.clone();
    no_alloc(|| s.audio(&mut l, &mut r, &p));
    let direct: Vec<f32> = input
        .iter()
        .map(|v| (v * 10f32.powf(0.9)).clamp(-1., 1.))
        .collect();
    let amp = |v: &[f32]| {
        let (mut re, mut im) = (0., 0.);
        for (i, x) in v[48000..].iter().enumerate() {
            let a = std::f64::consts::TAU * 13000. * i as f64 / 48000.;
            re += *x as f64 * a.cos();
            im += *x as f64 * a.sin();
        }
        (re * re + im * im).sqrt() / 48000.
    };
    let reduction = 20. * (amp(&l) / amp(&direct)).log10();
    assert!(
        reduction < -25.,
        "aliased harmonic reduction: {reduction} dB"
    );
}
#[test]
fn shaper_editing_automation_stereo_and_reset() {
    let p = Arc::new(FxParams::new(&specs::SHAPER));
    let panel = shaper::Panel::new(p.clone());
    assert!(panel.click(329., 169., None));
    assert_eq!(p.v(6), -1000);
    assert!(panel.click(328., 250., None));
    assert!(panel.dragging());
    assert!(p.v(9) > 0);
    assert!(panel.character('\u{1b}'));
    assert_eq!(p.v(9), 0);
    panel.click(328., 250., None);
    panel.moved(328., 300.);
    panel.release(None);
    assert_ne!(p.v(9), 0);
    panel.click(28., 180., None);
    assert_eq!(p.v(9), 0);
    for rate in [8000., 44100., 48000., 96000., 192000.] {
        let mut s = shaper::Shaper::default();
        s.prepare(rate, &p);
        for n in 0..12 {
            set(&p, 0, n * 30);
            set(&p, 1, if n % 2 == 0 { -1000 } else { 1000 });
            set(&p, 9, if n % 2 == 0 { -1000 } else { 1000 });
            let mut l = [0.3; 512];
            let mut r = [-0.2; 512];
            no_alloc(|| s.audio(&mut l, &mut r, &p));
            assert!(l.iter().chain(&r).all(|x| x.is_finite() && x.abs() < 4.));
        }
        set(&p, 1, 0);
        set(&p, 9, 0);
        no_alloc(|| s.reset(&p));
        let mut l = [0.; 512];
        let mut r = l;
        no_alloc(|| s.audio(&mut l, &mut r, &p));
        assert_eq!(rms(&l), 0.);
    }
}
fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::var("ML_TOOLS_RENDER_DIR").expect("tests need E: output directory");
    let path = std::path::Path::new(&root).join(format!("{name}.wav"));
    let s = tone(48000 * 4, 48000., 1000., 0.4);
    let data: Vec<f32> = s.iter().flat_map(|x| [*x, *x]).collect();
    os_engine::wav::write_f32_stereo(&mut std::fs::File::create(&path).unwrap(), 48000, &data)
        .unwrap();
    path
}
fn wait_load(c: &mut reference::Reference) {
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut l = [0.; 64];
        let mut r = l;
        no_alloc(|| c.audio(&mut l, &mut r, false, Some(0)));
        if c.loaded() {
            return;
        }
    }
    panic!("reference did not load");
}
#[test]
fn reference_match_reload_missing_and_callback_safety() {
    let mut c = reference::Reference::default();
    c.prepare(48000);
    let path = fixture("reference-direct");
    let panel = reference::Panel {
        shared: c.params.shared.clone(),
    };
    let before = c.params.serialize_fields();
    panel.file_dropped(&path);
    assert_eq!(panel.take_state_edits().len(), 1);
    wait_load(&mut c);
    let saved = c.params.serialize_fields();
    assert_ne!(saved, before);
    let input = tone(48000, 48000., 1000., 0.1);
    let mut l = input.clone();
    let mut r = l.clone();
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(0)));
    assert_eq!(l, input);
    c.params.shared.listen.store(true, Ordering::Release);
    l.copy_from_slice(&input);
    r.copy_from_slice(&input);
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(48000)));
    assert!((rms(&l[4800..]) - 0.1 / 2f64.sqrt()).abs() < 0.002);
    c.offline = true;
    l.copy_from_slice(&input);
    r.copy_from_slice(&input);
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(96000)));
    assert_eq!(l, input);
    c.offline = false;
    c.params.deserialize_fields(&before);
    c.params.deserialize_fields(&saved);
    assert_eq!(c.params.serialize_fields(), saved);
    assert!(!c.params.shared.listen.load(Ordering::Acquire));
    assert!(panel.take_state_edits().is_empty());
    wait_load(&mut c);
    c.params.shared.request(
        path.with_extension("missing")
            .to_string_lossy()
            .into_owned(),
        true,
    );
    std::thread::sleep(std::time::Duration::from_millis(80));
    l.copy_from_slice(&input);
    r.copy_from_slice(&input);
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(0)));
    assert_eq!(l, input);
    assert!(c.status().contains("Missing file"));
    c.params
        .shared
        .request(path.to_string_lossy().into_owned(), true);
    wait_load(&mut c);
    set(&c.params.fx, 0, 12000);
    c.params.shared.listen.store(true, Ordering::Release);
    l.copy_from_slice(&input);
    r.copy_from_slice(&input);
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(0)));
    assert_eq!(l, input);
}
mod a {
    type Effect = super::split::Split;
    os_plugkit::linked_clap_entry!(Effect);
}
mod b {
    type Effect = super::shaper::Waveshaper;
    os_plugkit::linked_clap_entry!(Effect);
}
mod c {
    type Effect = super::reference::Reference;
    os_plugkit::linked_clap_entry!(Effect);
}
#[test]
fn actual_host_state_latency_and_reference_offline_export() {
    use os_engine::HostedEffect as _;
    use os_host::Plugin as _;
    for (entry, name, latency) in [
        (&a::LINKED_CLAP_ENTRY, "ML Split", 0),
        (&b::LINKED_CLAP_ENTRY, "ML Waveshaper", 32),
        (&c::LINKED_CLAP_ENTRY, "ML Reference", 0),
    ] {
        let lib = unsafe {
            os_clap::Library::from_entry(
                entry as *const _ as *const std::ffi::c_void,
                std::path::Path::new("ML mix.clap"),
            )
        }
        .unwrap();
        assert_eq!(lib.plugins[0].name, name);
        let mut p = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
        assert!(p.activate(48000));
        assert_eq!(p.latency(), latency);
        let shared = reference::TEST_SHARED.with(|s| s.borrow().clone());
        if name == "ML Reference" {
            let shared = shared.unwrap();
            shared.request(
                fixture("reference-host").to_string_lossy().into_owned(),
                true,
            );
            std::thread::sleep(std::time::Duration::from_millis(150));
            let input = tone(48000, 48000., 500., 0.1);
            for block in 0..3 {
                let mut l = input.clone();
                let mut r = l.clone();
                p.transport_fx(&os_engine::TransportInfo {
                    playing: true,
                    frame: block * 48000,
                    tempo: 120.,
                    numerator: 4,
                    denominator: 4,
                    beats: block as f64 * 2.,
                    bar_start: 0.,
                    bar: 0,
                    looping: None,
                });
                p.process_fx(48000, &mut l, &mut r);
                shared.listen.store(true, Ordering::Release);
                if block == 2 {
                    assert!(
                        l.iter()
                            .zip(&input)
                            .skip(4000)
                            .any(|(a, b)| (a - b).abs() > 0.1)
                    );
                }
            }
            p.set_offline(true);
            let mut l = input.clone();
            let mut r = l.clone();
            p.process_fx(48000, &mut l, &mut r);
            assert_eq!(l, input, "offline render must never contain reference");
            p.set_offline(false);
        } else {
            let id = p.param_info(0).unwrap().id;
            p.set_param(id, 0.7);
            let mut l = tone(4096, 48000., 400., 0.1);
            let mut r = l.clone();
            p.process_fx(48000, &mut l, &mut r);
            p.idle();
        }
        let saved = p.save_state().unwrap();
        let mut restore = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
        assert!(restore.load_state(&saved));
        assert!(restore.activate(48000));
        assert_eq!(restore.latency(), latency);
        for i in 0..p.param_count() {
            let id = p.param_info(i).unwrap().id;
            assert_eq!(p.param_value(id), restore.param_value(id));
        }
    }
}
#[test]
fn editor_renders_and_parameter_text() {
    struct Model(&'static Spec);
    impl ml_small_fx::view::Model for Model {
        fn norm(&self, id: &str) -> f32 {
            let p = &self.0.settings[self.0.index(id).unwrap()];
            (p.start - p.range.0) as f32 / (p.range.1 - p.range.0) as f32
        }
        fn plain(&self, id: &str) -> f32 {
            self.0.settings[self.0.index(id).unwrap()].start as f32
        }
        fn text(&self, id: &str) -> String {
            let i = self.0.index(id).unwrap();
            (self.0.text)(i, self.0.settings[i].start)
        }
    }
    let reference = reference::Reference::default();
    for spec in [&specs::SPLIT, &specs::SHAPER, &specs::REFERENCE] {
        for (i, p) in spec.settings.iter().enumerate() {
            for v in [p.range.0, p.start, p.range.1] {
                assert_eq!(
                    (spec.parse)(i, &(spec.text)(i, v)),
                    Some(v),
                    "{} {}",
                    spec.name,
                    p.id
                );
            }
        }
        let p = Arc::new(FxParams::new(spec));
        let panel: Arc<dyn Extension> = match spec.name {
            "ML Split" => split::Splitter::panel(p),
            "ML Waveshaper" => shaper::Shaper::panel(p),
            _ => Arc::new(reference::Panel {
                shared: reference.params.shared.clone(),
            }),
        };
        for scale in [1., 1.5, 2.] {
            let mut canvas = Canvas::new(
                (spec.size.0 as f32 * scale) as u32,
                (spec.size.1 as f32 * scale) as u32,
                scale,
            )
            .unwrap();
            let model = Model(spec);
            ml_small_fx::view::paint(
                spec,
                &mut canvas,
                &os_theme::Palette::default(),
                &model,
                &Default::default(),
                "",
            );
            panel.paint(&mut canvas, &model);
            canvas
                .pixmap
                .save_png(
                    std::path::Path::new(&std::env::var("ML_TOOLS_RENDER_DIR").unwrap())
                        .join(format!("{} {scale}.png", spec.name)),
                )
                .unwrap();
        }
    }
}
#[test]
fn reference_file_change_fades_old_audio_before_replacement() {
    let mut c = reference::Reference::default();
    c.prepare(48000);
    set(&c.params.fx, 3, 0);
    c.params.shared.request(
        fixture("reference-transition")
            .to_string_lossy()
            .into_owned(),
        false,
    );
    wait_load(&mut c);
    c.params.shared.listen.store(true, Ordering::Release);
    let mut l = [0.1; 4096];
    let mut r = l;
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(1000)));
    c.params.shared.request(String::new(), true);
    l.fill(0.1);
    r.fill(0.1);
    no_alloc(|| c.audio(&mut l, &mut r, true, Some(5096)));
    assert!(
        l[0] > 0.3,
        "old reference should continue through the fade: {}",
        l[0]
    );
    assert_eq!(l[2048], 0.1, "finished fade must restore the exact mix");
}
