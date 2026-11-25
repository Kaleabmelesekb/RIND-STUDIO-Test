// SPDX-License-Identifier: GPL-3.0-or-later
// Airwindows: 139 of Chris Johnson's effects (the ones he lists as Recommended or Basic) as one effect plugin. The DSP is
// Airwindows Consolidated (`cpp/`: MIT, https://github.com/baconpaul/airwin2rack, from Airwindows, https://www.airwindows.com)
// with Honeydew's glue; the editor and the settings are Honeydew's.
#![allow(semicolon_in_expressions_from_non_local_macros)]
use ml_mix_core::{Fx, Unit};
use ml_small_fx::{Mark, Setting, Spec, editor::Extension, params::FxParams};
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
use std::{
    os::raw::{c_char, c_int, c_void},
    sync::Arc,
};

pub mod list;
mod ui;

use list::FX;

pub type Effect = Fx<Airwindows>;

unsafe extern "C" {
    fn aw_count() -> c_int;
    fn aw_new(index: c_int, rate: f32) -> *mut c_void;
    fn aw_free(o: *mut c_void);
    fn aw_set(o: *mut c_void, p: c_int, value: f32);
    fn aw_display(o: *mut c_void, p: c_int, out: *mut c_char, n: c_int);
    fn aw_process(o: *mut c_void, left: *mut f32, right: *mut f32, frames: c_int);
}

/// The most parameters an effect has.
pub const PARAMS: usize = 10;
/// The setting that holds the effect (its place in `list::FX`); the parameters follow.
pub const FX_SETTING: usize = 0;

/// The effect a new instance has: ToTape9.
pub const START_FX: i32 = 112;

pub static SETTINGS: [Setting; 11] = [
    Setting { id: "fx", name: "Effect", range: (0, 138), start: START_FX, magnet: None },
    Setting { id: "p1", name: "Parameter 1", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p2", name: "Parameter 2", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p3", name: "Parameter 3", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p4", name: "Parameter 4", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p5", name: "Parameter 5", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p6", name: "Parameter 6", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p7", name: "Parameter 7", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p8", name: "Parameter 8", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p9", name: "Parameter 9", range: (0, 1000), start: 500, magnet: None },
    Setting { id: "p10", name: "Parameter 10", range: (0, 1000), start: 500, magnet: None },
];

pub static IDS: [&str; 11] = ["fx", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10"];

pub static SPEC: Spec = Spec {
    name: "Airwindows",
    word: "AIRWINDOWS",
    about: "139 of Airwindows' effects, from tape and consoles to reverbs and filters, in one plugin.",
    settings: &SETTINGS,
    studio_settings: 0,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 470),
    cards: &[],
    controls: &[],
    display: None,
    mark: Mark { x: 744., y: 458., anchor: 1., stacked: false },
    hint: None,
    labels: &[],
    text,
    parse,
    sound: |_, _| unreachable!(),
    rings: true,
};

/// The effect a stored value means.
pub fn fx_at(v: i32) -> usize {
    (v.max(0) as usize).min(FX.len() - 1)
}

fn text(k: usize, v: i32) -> String {
    if k == FX_SETTING {
        FX[fx_at(v)].name.to_string()
    } else {
        format!("{:.1}%", f64::from(v) / 10.)
    }
}

fn parse(k: usize, t: &str) -> Option<i32> {
    let word = t.split_whitespace().next()?;
    if k == FX_SETTING {
        return FX.iter().position(|f| f.name.eq_ignore_ascii_case(word)).map(|i| i as i32);
    }
    let n = word.trim_end_matches('%').parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(((n * 10.).round() as i32).clamp(0, 1000))
}

pub(crate) struct Engine(*mut c_void);
// The effect is used from the audio thread only (the editor has an instance of its own).
unsafe impl Send for Engine {}
impl Engine {
    pub(crate) fn new(index: usize, rate: f32) -> Self {
        Engine(unsafe { aw_new(index as c_int, rate) })
    }
    pub(crate) fn set(&self, p: usize, v: f32) {
        if !self.0.is_null() {
            unsafe { aw_set(self.0, p as c_int, v) }
        }
    }
    /// Parameter `p`'s value as the effect shows it, with its unit.
    pub(crate) fn display(&self, p: usize) -> String {
        if self.0.is_null() {
            return String::new();
        }
        let mut buf = [0 as c_char; 96];
        unsafe { aw_display(self.0, p as c_int, buf.as_mut_ptr(), buf.len() as c_int) };
        let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { aw_free(self.0) }
        }
    }
}

pub struct Airwindows {
    rate: f32,
    engine: Option<(usize, Engine)>,
    /// The parameters as last sent to the effect (thousandths).
    sent: [i32; PARAMS],
}
impl Default for Airwindows {
    fn default() -> Self {
        Self { rate: 44100., engine: None, sent: [-1; PARAMS] }
    }
}
impl Airwindows {
    /// Makes the effect the settings name, if it is not the one running.
    fn follow(&mut self, p: &FxParams) {
        let want = fx_at(p.v(FX_SETTING));
        if self.engine.as_ref().is_none_or(|(i, _)| *i != want) {
            self.engine = Some((want, Engine::new(want, self.rate)));
            self.sent = [-1; PARAMS];
        }
        if let Some((_, e)) = &self.engine {
            for k in 0..PARAMS {
                let v = p.v(1 + k);
                if self.sent[k] != v {
                    e.set(k, v as f32 / 1000.);
                    self.sent[k] = v;
                }
            }
        }
    }
}
impl Unit for Airwindows {
    const SPEC: &'static Spec = &SPEC;
    const ID: &'static str = "org.honeydew.rind-airwindows";
    const CLASS: [u8; 16] = *b"RindAirwindows01";
    fn prepare(&mut self, rate: f32, p: &FxParams) {
        self.rate = rate;
        self.engine = None;
        self.follow(p);
    }
    fn reset(&mut self, p: &FxParams) {
        // A new effect starts from silence: what an effect holds (a delay line, a reverb tail) goes.
        self.engine = None;
        self.follow(p);
    }
    fn audio(&mut self, l: &mut [f32], r: &mut [f32], p: &FxParams) {
        self.follow(p);
        let Some((_, e)) = &self.engine else { return };
        if e.0.is_null() {
            return;
        }
        let n = l.len().min(r.len());
        let mut at = 0;
        while at < n {
            let len = (n - at).min(512);
            unsafe { aw_process(e.0, l[at..].as_mut_ptr(), r[at..].as_mut_ptr(), len as c_int) };
            at += len;
        }
        for x in l[..n].iter_mut().chain(&mut r[..n]) {
            if !x.is_finite() {
                *x = 0.;
            }
        }
    }
    fn panel(p: Arc<FxParams>) -> Arc<dyn Extension> {
        ui::Panel::new(p)
    }
}

#[cfg(not(feature = "linked"))]
nih_export_clap!(Effect);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(Effect);
os_plugkit::linked_clap_entry!(Effect);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_matches_the_engine() {
        assert_eq!(unsafe { aw_count() } as usize, FX.len());
        assert_eq!(SETTINGS[FX_SETTING].range.1 as usize, FX.len() - 1);
        assert_eq!(FX[START_FX as usize].name, "ToTape9");
        assert!(FX.iter().all(|f| f.params.len() <= PARAMS));
    }

    #[test]
    fn every_effect_runs_and_stays_finite() {
        for (i, f) in FX.iter().enumerate() {
            let e = Engine::new(i, 48000.);
            assert!(!e.0.is_null(), "{}", f.name);
            for (k, &(_, d)) in f.params.iter().enumerate() {
                e.set(k, f32::from(d) / 1000.);
            }
            let mut l: Vec<f32> = (0..2048).map(|n| ((n as f32) * 0.05).sin() * 0.5).collect();
            let mut r = l.clone();
            for _ in 0..4 {
                unsafe { aw_process(e.0, l.as_mut_ptr(), r.as_mut_ptr(), l.len() as c_int) };
            }
            assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{}", f.name);
        }
    }
}
