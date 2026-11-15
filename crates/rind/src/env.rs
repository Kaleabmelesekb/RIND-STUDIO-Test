// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! ADSR envelopes with exponential segments, and the low-frequency oscillators.

use crate::util::{Rng, fast_sin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// The per-tick smoothing amounts of an envelope's stages, for one tick (a sample, or a control step).
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvCoef {
    att: f32,
    dec: f32,
    rel: f32,
    sus: f32,
}

/// The share of the way to the target covered in `seconds` of exponential approach, taking `ticks` ticks per second.
fn alpha(seconds: f32, rate: f32, reach: f32) -> f32 {
    1.0 - (-reach / (seconds.max(1e-4) * rate)).exp()
}

impl EnvCoef {
    /// For times in seconds and `rate` ticks a second (the attack aims past 1 so it arrives; decay and release cover 60 dB
    /// in their time).
    pub fn new(att: f32, dec: f32, sus: f32, rel: f32, rate: f32) -> EnvCoef {
        EnvCoef {
            att: alpha(att, rate, 1.466),
            dec: alpha(dec, rate, 6.9),
            rel: alpha(rel, rate, 6.9),
            sus,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub level: f32,
    stage: Stage,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            level: 0.0,
            stage: Stage::Idle,
        }
    }
}

impl Env {
    /// Starts the attack from where the level is (no click when a voice is taken over).
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    pub fn is_released(&self) -> bool {
        matches!(self.stage, Stage::Release | Stage::Idle)
    }

    pub fn reset(&mut self) {
        *self = Env::default();
    }

    #[inline]
    pub fn tick(&mut self, c: &EnvCoef) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += (1.3 - self.level) * c.att;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level += (c.sus - self.level) * c.dec;
                if (self.level - c.sus).abs() < 1e-4 {
                    self.level = c.sus;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level += (c.sus - self.level) * c.dec,
            Stage::Release => {
                self.level -= self.level * c.rel;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

/// A low-frequency oscillator's state, advanced once per control step.
#[derive(Clone, Copy, Debug)]
pub struct Lfo {
    pub phase: f32,
    held: f32,
    prev: f32,
    next: f32,
    rng: Rng,
}

impl Lfo {
    pub fn new(seed: u32) -> Lfo {
        Lfo {
            phase: 0.0,
            held: 0.0,
            prev: 0.0,
            next: 0.0,
            rng: Rng::new(seed),
        }
    }

    /// Restarts at `phase` with fresh random values.
    pub fn start(&mut self, phase: f32) {
        self.phase = phase;
        self.held = self.rng.bipolar();
        self.prev = self.held;
        self.next = self.rng.bipolar();
    }

    /// Moves on by `inc` of a cycle.
    pub fn advance(&mut self, inc: f32) {
        self.phase += inc;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.held = self.rng.bipolar();
            self.prev = self.next;
            self.next = self.rng.bipolar();
        }
    }

    /// The wave (see `LFO_WAVE_NAMES`) in `[-1, 1]`, `offset` of a cycle ahead.
    pub fn value(&self, wave: u8, offset: f32) -> f32 {
        let p = (self.phase + offset).rem_euclid(1.0);
        match wave {
            0 => fast_sin(p),
            1 => 1.0 - 4.0 * ((p + 0.25).rem_euclid(1.0) - 0.5).abs(),
            2 => 2.0 * p - 1.0,
            3 => 1.0 - 2.0 * p,
            4 => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            5 => {
                if p < 0.25 {
                    1.0
                } else {
                    -1.0
                }
            }
            6 => {
                if p < 0.125 {
                    1.0
                } else {
                    -1.0
                }
            }
            7 => self.held,
            8 => {
                let t = self.phase * self.phase * (3.0 - 2.0 * self.phase);
                self.prev + (self.next - self.prev) * t
            }
            9 => (p * 4.0).floor() / 3.0 * 2.0 - 1.0,
            _ => fast_sin(p).abs() * 2.0 - 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adsr_rises_holds_and_releases() {
        let sr = 48_000.0;
        let c = EnvCoef::new(0.01, 0.05, 0.5, 0.1, sr);
        let mut e = Env::default();
        assert!(e.is_idle());
        e.gate_on();
        let mut peak = 0.0f32;
        for _ in 0..4800 {
            peak = peak.max(e.tick(&c));
        }
        assert!((peak - 1.0).abs() < 1e-3, "{peak}");
        for _ in 0..48_000 {
            e.tick(&c);
        }
        assert!((e.level - 0.5).abs() < 1e-3, "sustain {}", e.level);
        e.gate_off();
        for _ in 0..48_000 {
            e.tick(&c);
        }
        assert!(e.is_idle() && e.level == 0.0);
    }

    #[test]
    fn retrigger_keeps_the_level() {
        let c = EnvCoef::new(0.05, 0.05, 1.0, 0.5, 48_000.0);
        let mut e = Env::default();
        e.gate_on();
        for _ in 0..2000 {
            e.tick(&c);
        }
        e.gate_off();
        for _ in 0..500 {
            e.tick(&c);
        }
        let before = e.level;
        e.gate_on();
        assert!(e.tick(&c) >= before * 0.99);
    }

    #[test]
    fn lfo_waves_stay_in_range() {
        for wave in 0..11 {
            let mut l = Lfo::new(3);
            l.start(0.0);
            for _ in 0..5000 {
                l.advance(0.0137);
                let v = l.value(wave, 0.2);
                assert!((-1.001..=1.001).contains(&v), "wave {wave}: {v}");
            }
        }
    }

    #[test]
    fn sine_lfo_starts_at_zero_and_rises() {
        let mut l = Lfo::new(1);
        l.start(0.0);
        assert!(l.value(0, 0.0).abs() < 1e-3);
        l.advance(0.1);
        assert!(l.value(0, 0.0) > 0.5);
    }

    // ---- Timing, retrigger and extremes, measured. ----

    /// Ticks until `level` first reaches `goal` (up or down), or `None` within `max` ticks.
    fn ticks_until(e: &mut Env, c: &EnvCoef, goal: f32, rising: bool, max: usize) -> Option<usize> {
        for i in 1..=max {
            let v = e.tick(c);
            if (rising && v >= goal) || (!rising && v <= goal) {
                return Some(i);
            }
        }
        None
    }

    #[test]
    fn the_stage_times_hold_at_every_sample_rate() {
        for rate in [22_050.0f32, 44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for time in [0.005f32, 0.05, 0.5, 2.0] {
                // Attack: full level after the attack time.
                let c = EnvCoef::new(time, 1.0, 0.0, 1.0, rate);
                let mut e = Env::default();
                e.gate_on();
                let n =
                    ticks_until(&mut e, &c, 1.0, true, (4.0 * time * rate) as usize + 10).unwrap();
                let secs = n as f32 / rate;
                assert!(
                    (secs / time - 1.0).abs() < 0.03,
                    "attack {time} s at {rate}: {secs}"
                );
                // Decay to zero sustain: 60 dB down after the decay time.
                let c = EnvCoef::new(0.0001, time, 0.0, 1.0, rate);
                let mut e = Env::default();
                e.gate_on();
                ticks_until(&mut e, &c, 1.0, true, 10_000).unwrap();
                let n = ticks_until(&mut e, &c, 1e-3, false, (4.0 * time * rate) as usize + 10)
                    .unwrap();
                let secs = n as f32 / rate;
                assert!(
                    (secs / time - 1.0).abs() < 0.05,
                    "decay {time} s at {rate}: {secs}"
                );
                // Release: 60 dB down after the release time, from any level.
                let c = EnvCoef::new(0.0001, 1.0, 1.0, time, rate);
                let mut e = Env::default();
                e.gate_on();
                for _ in 0..(0.02 * rate) as usize {
                    e.tick(&c);
                }
                e.gate_off();
                let from = e.level;
                let n = ticks_until(
                    &mut e,
                    &c,
                    from * 1e-3,
                    false,
                    (4.0 * time * rate) as usize + 10,
                )
                .unwrap();
                let secs = n as f32 / rate;
                assert!(
                    (secs / time - 1.0).abs() < 0.05,
                    "release {time} s at {rate}: {secs}"
                );
            }
        }
    }

    #[test]
    fn the_segments_move_one_way_and_never_leave_the_range() {
        let c = EnvCoef::new(0.01, 0.1, 0.4, 0.2, 48_000.0);
        let mut e = Env::default();
        e.gate_on();
        let mut prev = 0.0f32;
        for _ in 0..480 {
            let v = e.tick(&c);
            assert!(
                v >= prev && v <= 1.0,
                "attack must rise to 1 and stop: {prev} -> {v}"
            );
            prev = v;
        }
        for _ in 0..48_000 {
            let v = e.tick(&c);
            assert!((0.4 - 1e-3..=1.0).contains(&v), "{v}");
        }
        e.gate_off();
        let mut prev = e.level;
        for _ in 0..96_000 {
            let v = e.tick(&c);
            assert!(v <= prev && v >= 0.0, "release only falls: {prev} -> {v}");
            prev = v;
        }
        assert!(e.is_idle());
    }

    #[test]
    fn a_retrigger_continues_from_the_level_it_finds_without_a_step() {
        let rate = 48_000.0;
        let c = EnvCoef::new(0.02, 0.2, 0.6, 0.3, rate);
        // Gate on again at any moment of any stage: the next sample is within a normal step of the last.
        for stop in [10usize, 400, 2_000, 9_000, 30_000] {
            for released in [false, true] {
                let mut e = Env::default();
                e.gate_on();
                for _ in 0..stop {
                    e.tick(&c);
                }
                if released {
                    e.gate_off();
                    for _ in 0..300 {
                        e.tick(&c);
                    }
                }
                let before = e.level;
                e.gate_on();
                let after = e.tick(&c);
                assert!(
                    (after - before).abs() < 0.05,
                    "stop {stop} released {released}: {before} -> {after}"
                );
                assert!(
                    after >= before - 1e-3,
                    "a new attack never starts by falling"
                );
            }
        }
    }

    #[test]
    fn releasing_during_the_attack_and_idle_gates_are_safe() {
        let c = EnvCoef::new(0.5, 0.2, 0.6, 0.3, 48_000.0);
        let mut e = Env::default();
        e.gate_off();
        assert!(
            e.is_idle() && e.tick(&c) == 0.0,
            "a release with nothing playing stays idle"
        );
        e.gate_on();
        for _ in 0..2_000 {
            e.tick(&c);
        }
        let mid = e.level;
        assert!(mid > 0.0 && mid < 1.0);
        e.gate_off();
        let next = e.tick(&c);
        assert!(
            next < mid && next > mid * 0.99,
            "the release starts from the level it was at: {mid} -> {next}"
        );
    }

    #[test]
    fn extreme_times_and_sustains_stay_finite() {
        for rate in [8_000.0f32, 48_000.0, 384_000.0] {
            for (a, d, s, r) in [
                (0.0, 0.0, 0.0, 0.0),
                (0.0, 0.0, 1.0, 0.0),
                (60.0, 60.0, 0.5, 60.0),
                (0.0001, 0.0001, 0.0, 0.0001),
                (1e-9, 1e9, 2.0, 1e9),
            ] {
                let c = EnvCoef::new(a, d, s, r, rate);
                let mut e = Env::default();
                e.gate_on();
                for i in 0..20_000 {
                    if i == 10_000 {
                        e.gate_off();
                    }
                    let v = e.tick(&c);
                    assert!(
                        v.is_finite() && (0.0..=2.0).contains(&v),
                        "{a} {d} {s} {r} at {rate}: {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn two_envelopes_are_independent_and_the_tail_ends_exactly() {
        let amp = EnvCoef::new(0.001, 0.3, 0.7, 0.05, 48_000.0);
        let filter = EnvCoef::new(0.2, 0.4, 0.1, 1.5, 48_000.0);
        let (mut a, mut f) = (Env::default(), Env::default());
        a.gate_on();
        f.gate_on();
        for _ in 0..2_000 {
            a.tick(&amp);
            f.tick(&filter);
        }
        assert!(
            a.level > 0.7 && f.level < 0.5 && f.level < a.level,
            "{} {}",
            a.level,
            f.level
        );
        a.gate_off();
        for _ in 0..48_000 {
            a.tick(&amp);
        }
        assert!(
            a.is_idle() && a.level == 0.0,
            "an ended envelope is exactly zero"
        );
        assert!(!f.is_idle(), "the other one is untouched");
    }
}
