use super::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct Alloc;
thread_local! {static COUNT:Cell<Option<usize>>=const{Cell::new(None)};}
#[global_allocator]
static ALLOC: Alloc = Alloc;
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
}
fn no_alloc(f: impl FnOnce()) {
    COUNT.with(|c| c.set(Some(0)));
    f();
    let n = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(n, 0, "Rust allocations in processing/reset");
}
fn set(p: &FxParams, index: usize, value: i32) {
    p.live
        .set(p.p[index].as_ptr(), p.p[index].preview_normalized(value));
}
fn sine(rate: u32, hz: f32, seconds: usize) -> Vec<f32> {
    (0..rate as usize * seconds)
        .map(|i| (i as f32 * std::f32::consts::TAU * hz / rate as f32).sin() * 0.1)
        .collect()
}
fn noise(n: usize) -> Vec<f32> {
    let mut seed = 71u32;
    (0..n)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed as i32 as f32 / i32::MAX as f32 * 0.5
        })
        .collect()
}
fn energy(b: &[f32]) -> f64 {
    b.iter().map(|x| f64::from(*x).powi(2)).sum()
}
fn freq(b: &[f32], rate: u32) -> f64 {
    let points: Vec<_> = b
        .windows(2)
        .enumerate()
        .filter(|(_, x)| x[0] < 0. && x[1] >= 0.)
        .map(|(i, _)| i)
        .collect();
    (points.len() - 1) as f64 * rate as f64 / (points.last().unwrap() - points[0]) as f64
}
#[test]
fn deesser_reuse_listen_and_zero_amount() {
    let p = FxParams::new(&specs::DEESSER);
    let data = Arc::new(Readings::default());
    let input = noise(48000);
    let mut left = input.clone();
    let mut right = input.clone();
    let mut d = deesser::DeEss::new(48000, data.clone());
    d.reset(&p);
    no_alloc(|| d.process(&mut left, &mut right, &p));
    assert_eq!(left, right);
    assert!(energy(&left) < energy(&input) * 0.95);
    assert!(data.get(0) > 0.);
    let mut reference: Vec<_> = input.iter().flat_map(|x| [*x, *x]).collect();
    os_dsp::debess::DeBess::new(48000).process(&mut reference, 0.5);
    assert!(
        left.iter()
            .zip(reference.chunks_exact(2))
            .all(|(x, y)| (*x - y[0]).abs() < 1e-6)
    );
    set(&p, 3, 1);
    d.reset(&p);
    let mut removed = input.clone();
    let mut rr = input.clone();
    d.process(&mut removed, &mut rr, &p);
    assert!(
        removed
            .iter()
            .zip(&left)
            .zip(&input)
            .all(|((a, b), x)| (a + b - x).abs() < 1e-6)
    );
    set(&p, 3, 0);
    set(&p, 0, 0);
    d.reset(&p);
    let mut dry = input.clone();
    d.process(&mut dry, &mut rr, &p);
    assert_eq!(dry, input);
}
#[test]
fn meter_is_transparent_calibrated_and_resets_without_allocations() {
    for rate in [44100, 48000, 96000, 192000] {
        let p = FxParams::new(&specs::METER);
        let data = Arc::new(Readings::default());
        let mut d = meter::Measure::new(rate, data.clone());
        d.reset(&p);
        let input = sine(rate, 1000., 4);
        let mut l = input.clone();
        let mut r = input.clone();
        no_alloc(|| d.process(&mut l, &mut r, &p));
        assert_eq!(l, input);
        assert_eq!(r, input);
        assert!((data.get(0) + 20.).abs() < 0.05);
        assert!((data.get(2) + 23.0103).abs() < 0.05);
        assert!(
            (data.get(6) + 20.).abs() < 0.2,
            "momentary {} at {rate}",
            data.get(6)
        );
        assert!((data.get(8) + 20.).abs() < 0.2);
        assert!(data.get(4) >= data.get(0) - 0.001);
        assert!((data.get(9) - 1.).abs() < 1e-5);
        no_alloc(|| d.reset(&p));
        assert!(data.get(8).is_infinite());
        assert_eq!(data.get(10), 0.);
        for x in &mut r {
            *x = -*x;
        }
        d.process(&mut l, &mut r, &p);
        assert!((data.get(9) + 1.).abs() < 1e-5);
        data.reset.fetch_add(1, Ordering::Release);
        let mut silence = vec![0.; rate as usize / 2];
        let mut sr = silence.clone();
        d.process(&mut silence, &mut sr, &p);
        assert!(data.get(8).is_infinite());
        assert_eq!(data.get(9), 0.);
    }
}
#[test]
fn pitch_moves_frequency_preserves_duration_and_reports_delay() {
    for rate in [44100, 48000, 96000] {
        for semitones in [-12, 12] {
            let p = FxParams::new(&specs::PITCH);
            set(&p, 0, semitones);
            let mut d = pitch::Pitch::new(rate, Arc::default());
            d.reset(&p);
            let mut l = sine(rate, 440., 2);
            let mut r = l.clone();
            no_alloc(|| d.process(&mut l, &mut r, &p));
            assert_eq!(l.len(), rate as usize * 2);
            assert!(l.iter().all(|x| x.is_finite()));
            let f = freq(&l[rate as usize..], rate);
            let expected = 440. * 2f64.powf(semitones as f64 / 12.);
            assert!((f - expected).abs() < 2., "{f} vs {expected}");
            // Signalsmith locks channel phases, but its floating-point channel
            // predictions are not bit-identical. Check image stability by energy.
            let a = &l[rate as usize..];
            let b = &r[rate as usize..];
            let residual = a
                .iter()
                .zip(b)
                .map(|(a, b)| f64::from(a - b).powi(2))
                .sum::<f64>();
            let relative = (residual / energy(a)).sqrt();
            let correlation = a
                .iter()
                .zip(b)
                .map(|(a, b)| f64::from(*a) * f64::from(*b))
                .sum::<f64>()
                / (energy(a) * energy(b)).sqrt();
            assert!(
                relative < 0.001 && correlation > 0.99999,
                "stereo residual {relative}, correlation {correlation}, rate {rate}, shift {semitones}"
            );
        }
    }
    for rate in [44100, 48000, 96000, 192000] {
        let p = FxParams::new(&specs::PITCH);
        let mut d = pitch::Pitch::new(rate, Arc::default());
        d.reset(&p);
        let delay = d.latency() as usize;
        let mut engine = signalsmith_stretch::Stretch::preset_default(2, rate);
        let mut impulse = vec![0.; (delay + 1024) * 2];
        impulse[0] = 1.;
        impulse[1] = 1.;
        let mut wet = vec![0.; impulse.len()];
        engine.process(&impulse, &mut wet);
        let peak = wet
            .chunks_exact(2)
            .enumerate()
            .max_by(|a, b| a.1[0].abs().partial_cmp(&b.1[0].abs()).unwrap())
            .unwrap()
            .0;
        assert_eq!(peak, delay, "wet and dry latency must agree at {rate}");
        let mut l = vec![0.; delay + 1024];
        let mut r = l.clone();
        l[0] = 0.5;
        r[0] = -0.25;
        d.process(&mut l, &mut r, &p);
        assert_eq!(l[delay], 0.5);
        assert_eq!(r[delay], -0.25);
        assert_eq!(l.iter().filter(|x| **x != 0.).count(), 1);
        set(&p, 0, 12);
        set(&p, 3, 0);
        d.reset(&p);
        l.fill(0.);
        r.fill(0.);
        l[0] = 0.5;
        d.process(&mut l, &mut r, &p);
        assert_eq!(l[delay], 0.5);
    }
}
mod a {
    type Effect = Stock<deesser::DeEsser>;
    use super::*;
    os_plugkit::linked_clap_entry!(Effect);
}
mod b {
    type Effect = Stock<meter::Meter>;
    use super::*;
    os_plugkit::linked_clap_entry!(Effect);
}
mod c {
    type Effect = Stock<pitch::PitchShift>;
    use super::*;
    os_plugkit::linked_clap_entry!(Effect);
}
#[test]
fn host_identity_latency_and_state() {
    use os_engine::HostedEffect as _;
    use os_host::Plugin as _;
    for (entry, name) in [
        (&a::LINKED_CLAP_ENTRY, "ML De-Esser"),
        (&b::LINKED_CLAP_ENTRY, "ML Meter"),
        (&c::LINKED_CLAP_ENTRY, "ML Pitch Shift"),
    ] {
        let lib = unsafe {
            os_clap::Library::from_entry(
                entry as *const _ as *const std::ffi::c_void,
                std::path::Path::new("ML tools.clap"),
            )
        }
        .unwrap();
        assert_eq!(lib.plugins[0].name, name);
        let mut p = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
        assert!(p.activate(48000));
        assert!(p.param_info(p.param_count()).is_none());
        assert!(p.param_info(usize::MAX).is_none());
        let latency = p.latency();
        if name == "ML Pitch Shift" {
            assert!(latency > 0);
        } else {
            assert_eq!(latency, 0);
        }
        let id = if p.param_count() > 0 {
            p.param_info(0).map(|x| x.id)
        } else {
            None
        };
        if let Some(id) = id {
            p.set_param(id, 0.75);
        }
        p.process_fx(48000, &mut [0.; 256], &mut [0.; 256]);
        p.idle();
        let state = p.save_state().unwrap();
        let mut restored = os_clap::Plugin::new(&lib, &lib.plugins[0].id).unwrap();
        assert!(restored.load_state(&state));
        assert!(restored.activate(48000));
        assert_eq!(restored.latency(), latency);
        if let Some(id) = id {
            assert!((restored.param_value(id).unwrap() - p.param_value(id).unwrap()).abs() < 1e-6);
        }
    }
}

#[test]
fn pitch_automation_and_meter_reset_gesture() {
    let p = FxParams::new(&specs::PITCH);
    let mut d = pitch::Pitch::new(48000, Arc::default());
    d.reset(&p);
    for step in 0..40 {
        set(&p, 0, if step % 2 == 0 { 24 } else { -24 });
        set(&p, 1, if step % 2 == 0 { 100 } else { -100 });
        set(&p, 2, step % 2);
        set(&p, 3, if step % 3 == 0 { 0 } else { 1000 });
        let mut l = [0.1; 512];
        let mut r = [-0.1; 512];
        no_alloc(|| d.process(&mut l, &mut r, &p));
        assert!(
            l.iter()
                .chain(r.iter())
                .all(|x| x.is_finite() && x.abs() < 1.)
        );
    }
    let data = Arc::new(Readings::default());
    data.set(8, -18.);
    data.set(10, 42.);
    let panel = Panel::<meter::Meter> {
        data: data.clone(),
        kind: PhantomData,
    };
    assert!(!panel.click(590., 10., None));
    assert!(panel.click(55., 300., None));
    assert_eq!(data.reset.load(Ordering::Acquire), 1);
    assert_eq!(data.get(10), 0.);
    assert!(data.get(8).is_infinite());
}
#[test]
fn editor_layouts_and_values() {
    struct Model(&'static Spec);
    impl ml_small_fx::view::Model for Model {
        fn norm(&self, id: &str) -> f32 {
            let s = &self.0.settings[self.0.index(id).unwrap()];
            (s.start - s.range.0) as f32 / (s.range.1 - s.range.0) as f32
        }
        fn plain(&self, id: &str) -> f32 {
            self.0.settings[self.0.index(id).unwrap()].start as f32
        }
        fn text(&self, id: &str) -> String {
            let i = self.0.index(id).unwrap();
            (self.0.text)(i, self.0.settings[i].start)
        }
    }
    for spec in [&specs::DEESSER, &specs::METER, &specs::PITCH] {
        for (i, s) in spec.settings.iter().enumerate() {
            for v in [s.range.0, s.start, s.range.1] {
                assert_eq!((spec.parse)(i, &(spec.text)(i, v)), Some(v));
            }
        }
        if let Ok(dir) = std::env::var("ML_TOOLS_RENDER_DIR") {
            for scale in [1., 1.5, 2.] {
                let mut c = Canvas::new(
                    (spec.size.0 as f32 * scale) as u32,
                    (spec.size.1 as f32 * scale) as u32,
                    scale,
                )
                .unwrap();
                let hits = ml_small_fx::view::paint(
                    spec,
                    &mut c,
                    &os_theme::Palette::default(),
                    &Model(spec),
                    &Default::default(),
                    "",
                );
                assert!(hits.len() >= spec.settings.len());
                let data = Readings::default();
                for i in 0..9 {
                    data.set(i, -18.);
                }
                data.set(9, 0.9);
                data.set(10, 42.);
                match spec.name {
                    "ML De-Esser" => {
                        data.set(0, 2.3);
                        deesser::DeEsser::paint(&mut c, &data)
                    }
                    "ML Meter" => meter::Meter::paint(&mut c, &data),
                    _ => {
                        data.set(0, 120.);
                        pitch::PitchShift::paint(&mut c, &data)
                    }
                }
                c.pixmap
                    .save_png(std::path::Path::new(&dir).join(format!("{} {scale}.png", spec.name)))
                    .unwrap();
            }
        }
    }
}
