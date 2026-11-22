//! Zero-allocation Stereo Waveguide Tube Resonator FX ported from `tube-resonator-dsp`.
//!
//! Pre-allocates fixed 16K-sample circular delay buffers per channel in `new()`/`initialize()`.
//! Integrates with `J-SYN`'s `Dirty` / `Clean` engine mode to toggle 32-sample stepped vs.
//! smooth per-sample Pitch LFO modulation.

use std::f64::consts::{FRAC_1_SQRT_2, PI, TAU};

use super::{flush_denormal, track_idle};

/// Maximum delay length (supports down to ~11.7 Hz at 192 kHz with power-of-two masking).
const MAX_DELAY_SAMPLES: usize = 16_384;
const DELAY_MASK: usize = MAX_DELAY_SAMPLES - 1;

const REF_FREQ_HZ: f64 = 44.0;
const END_CORRECTION_RATIO: f64 = 1.006;
const REF_EXCITATION_GAIN: f64 = -5.88;
const CONTROL_STEP_SAMPLES: u32 = 32;

pub const TUBE_RES_MIN_MIDI: f32 = 12.0; // C0 (16.35 Hz)
pub const TUBE_RES_MAX_MIDI: f32 = 84.0; // C6 (1046.50 Hz)
pub const TUBE_RES_MIDI_SPAN: f32 = TUBE_RES_MAX_MIDI - TUBE_RES_MIN_MIDI; // 72 semitones

/// Convert normalized `P1` (`0..=1`) to continuous MIDI note (`12.0 = C0` .. `84.0 = C6`).
#[inline]
pub fn tube_res_tune_midi(norm: f32) -> f32 {
    TUBE_RES_MIN_MIDI + norm.clamp(0.0, 1.0) * TUBE_RES_MIDI_SPAN
}

/// Convert normalized `P1` (`0..=1`) to fundamental frequency in Hz (`A4 = 69 = 440 Hz`).
#[inline]
pub fn tube_res_tune_hz(norm: f32) -> f32 {
    let midi = tube_res_tune_midi(norm);
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Format an integer MIDI note number (`0..=127`) as a standard note name (`C0`..`C6`).
pub fn midi_note_name(note: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let idx = note.rem_euclid(12) as usize;
    let octave = note.div_euclid(12) - 1;
    format!("{}{}", NAMES[idx], octave)
}

/// Format normalized `TUNE` as `"C1"` on exact semitones, or `"C1 +15c"` when between notes.
pub fn format_tube_res_tune(norm: f32) -> String {
    let midi = tube_res_tune_midi(norm);
    let nearest = midi.round() as i32;
    let cents = ((midi - nearest as f32) * 100.0).round() as i32;
    let note = midi_note_name(nearest);
    if cents == 0 {
        note
    } else {
        format!("{note} {cents:+}c")
    }
}

/// Convert normalized `P2` (`0..=1`) to loop softening filter center frequency (`20 Hz..=20 kHz`, skew `-2.2`).
#[inline]
pub fn tube_res_flt_freq_hz(norm: f32) -> f32 {
    20.0 + 19_980.0 * norm.clamp(0.0, 1.0).powf(4.594_793_4)
}

/// Convert normalized `P3` (`0..=1`) to softening filter bandwidth (`0.5..=9.0` octaves).
#[inline]
pub fn tube_res_flt_width_oct(norm: f32) -> f32 {
    0.5 + 8.5 * norm.clamp(0.0, 1.0)
}

/// Convert normalized `P6` (`0..=1`) to Pitch LFO rate (`0.01..=30.0 Hz`, skew `-2.0`).
#[inline]
pub fn tube_res_lfo_rate_hz(norm: f32) -> f32 {
    0.01 + 29.99 * norm.clamp(0.0, 1.0).powi(4)
}

/// Convert normalized `P7` (`0..=1`) to waveguide decay time (`10.0..=5000.0 ms`, skew `-2.0`).
#[inline]
pub fn tube_res_decay_ms(norm: f32) -> f32 {
    10.0 + 4_990.0 * norm.clamp(0.0, 1.0).powi(4)
}

#[derive(Clone, Copy, Debug, Default)]
struct Biquad64 {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad64 {
    fn set_highpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        let fc = cutoff_hz.clamp(1.0, 0.495 * sample_rate);
        let w0 = 2.0 * PI * fc / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0_inv = 1.0 / (1.0 + alpha);

        let b0 = (1.0 + cos_w0) * 0.5 * a0_inv;
        self.b0 = b0;
        self.b1 = -(1.0 + cos_w0) * a0_inv;
        self.b2 = b0;
        self.a1 = (-2.0 * cos_w0) * a0_inv;
        self.a2 = (1.0 - alpha) * a0_inv;
    }

    fn set_lowpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        let fc = cutoff_hz.clamp(1.0, 0.495 * sample_rate);
        let w0 = 2.0 * PI * fc / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0_inv = 1.0 / (1.0 + alpha);

        let b0 = (1.0 - cos_w0) * 0.5 * a0_inv;
        self.b0 = b0;
        self.b1 = (1.0 - cos_w0) * a0_inv;
        self.b2 = b0;
        self.a1 = (-2.0 * cos_w0) * a0_inv;
        self.a2 = (1.0 - alpha) * a0_inv;
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[derive(Clone, Debug, Default)]
struct SofteningBandpass {
    hp: Biquad64,
    lp: Biquad64,
    gain: f64,
    last_freq_hz: f64,
    last_bw_oct: f64,
    last_sr: f64,
}

impl SofteningBandpass {
    fn update(&mut self, freq_hz: f64, bandwidth_oct: f64, sample_rate: f64) {
        if (freq_hz - self.last_freq_hz).abs() < 1e-4
            && (bandwidth_oct - self.last_bw_oct).abs() < 1e-4
            && (sample_rate - self.last_sr).abs() < 1e-3
        {
            return;
        }
        self.last_freq_hz = freq_hz;
        self.last_bw_oct = bandwidth_oct;
        self.last_sr = sample_rate;

        let half_bw = 0.5 * bandwidth_oct.clamp(0.1, 12.0);
        let f_center = freq_hz * 1.015;
        let f_hp = (f_center * (-half_bw).exp2()).clamp(20.0, 20_000.0);
        let f_lp = (f_center * half_bw.exp2()).clamp(20.0, 20_000.0);

        self.hp.set_highpass(f_hp, FRAC_1_SQRT_2, sample_rate);
        self.lp.set_lowpass(f_lp, FRAC_1_SQRT_2, sample_rate);
        self.gain = 1.0045;
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y_hp = self.hp.process(x);
        self.gain * self.lp.process(y_hp)
    }

    fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
        self.last_sr = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
struct DcBlocker29 {
    r: f64,
    x1: f64,
    y1: f64,
}

impl DcBlocker29 {
    fn new(cutoff_hz: f64, sample_rate: f64) -> Self {
        let mut s = Self {
            r: 0.995_735,
            x1: 0.0,
            y1: 0.0,
        };
        s.set_cutoff(cutoff_hz, sample_rate);
        s
    }

    fn set_cutoff(&mut self, cutoff_hz: f64, sample_rate: f64) {
        let fc = cutoff_hz.clamp(1.0, 200.0);
        self.r = (1.0 - (2.0 * PI * fc / sample_rate)).clamp(0.9, 0.9999);
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
struct PitchLfo {
    phase: f64,
    step_counter: u32,
    held: f64,
}

impl PitchLfo {
    fn new() -> Self {
        Self {
            phase: 0.0,
            step_counter: 0,
            held: 1.0,
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.step_counter = 0;
        self.held = 1.0;
    }

    #[inline]
    fn next_multiplier(
        &mut self,
        amount_0_to_1: f64,
        rate_hz: f64,
        dirty: bool,
        sample_rate: f64,
    ) -> (f64, f32) {
        if !dirty {
            self.step_counter = 0;
            let cur_phase = self.phase;
            self.phase = (self.phase + TAU * rate_hz / sample_rate).rem_euclid(TAU);
            if amount_0_to_1 <= 0.0 {
                return (1.0, cur_phase as f32);
            }
            let mod_oct = amount_0_to_1.clamp(0.0, 1.0) * cur_phase.sin();
            return (mod_oct.exp2(), cur_phase as f32);
        }

        let cur_phase = self.phase;
        if self.step_counter == 0 {
            if amount_0_to_1 <= 0.0 {
                self.held = 1.0;
            } else {
                let mod_oct = amount_0_to_1.clamp(0.0, 1.0) * cur_phase.sin();
                self.held = mod_oct.exp2();
            }
        }
        self.phase = (self.phase + TAU * rate_hz / sample_rate).rem_euclid(TAU);
        self.step_counter = (self.step_counter + 1) % CONTROL_STEP_SAMPLES;
        (self.held, cur_phase as f32)
    }
}

/// Pre-allocated single-channel fractional-delay waveguide resonator (`Box<[f64]>`).
#[derive(Clone, Debug)]
struct WaveguideResonator {
    buffer: Box<[f64]>,
    write_pos: usize,
    lp_state: f64,
}

impl WaveguideResonator {
    fn new() -> Self {
        Self {
            buffer: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            write_pos: 0,
            lp_state: 0.0,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.lp_state = 0.0;
    }

    #[inline]
    fn read_cubic(&self, delay_samples: f64) -> f64 {
        let d = delay_samples.clamp(2.0, (MAX_DELAY_SAMPLES - 4) as f64);
        let d_int = d.floor() as usize;
        let frac = d - (d_int as f64);

        let i_m1 = self.write_pos.wrapping_sub(d_int - 1) & DELAY_MASK;
        let i_0 = self.write_pos.wrapping_sub(d_int) & DELAY_MASK;
        let i_1 = self.write_pos.wrapping_sub(d_int + 1) & DELAY_MASK;
        let i_2 = self.write_pos.wrapping_sub(d_int + 2) & DELAY_MASK;

        let ym1 = self.buffer[i_m1];
        let y0 = self.buffer[i_0];
        let y1 = self.buffer[i_1];
        let y2 = self.buffer[i_2];

        let c0 = y0;
        let c1 = 0.5 * (y1 - ym1);
        let c2 = ym1 - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
        let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);

        ((c3 * frac + c2) * frac + c1) * frac + c0
    }

    #[inline]
    fn process_sample(
        &mut self,
        input: f64,
        tune_hz: f64,
        decay_s: f64,
        damping_0_1: f64,
        sample_rate: f64,
    ) -> f64 {
        let f0 = (tune_hz * END_CORRECTION_RATIO).clamp(15.0, 5_000.0);
        let period_samples = sample_rate / f0;
        let half_period_samples = 0.5 * period_samples;

        let decay_clamped = decay_s.max(0.005);
        let g_decay = (-4.0 / (f0 * decay_clamped)).exp();

        // Loop high-frequency retention (`1.0` = 100% default retention, lower = darker tail)
        let damping = damping_0_1.clamp(0.0, 1.0);
        let lp_a = if damping >= 0.999 {
            0.22 * (sample_rate / 44_100.0).recip().clamp(0.1, 0.9)
        } else {
            let lp_cutoff = 400.0 * (40.0_f64).powf(damping);
            (-2.0 * PI * lp_cutoff / sample_rate).exp().clamp(0.0, 0.995)
        };
        let loop_delay_comp = lp_a / (1.0 - lp_a);

        let delayed_full = self.read_cubic((period_samples - loop_delay_comp).max(2.0));
        self.lp_state = (1.0 - lp_a) * delayed_full + lp_a * self.lp_state;

        self.buffer[self.write_pos] = input + g_decay * self.lp_state;

        let delayed_half = self.read_cubic((half_period_samples - 0.5 * loop_delay_comp).max(2.0));
        let pitch_norm_gain = REF_EXCITATION_GAIN * (REF_FREQ_HZ / f0);
        let out = pitch_norm_gain * delayed_half;

        self.write_pos = (self.write_pos + 1) & DELAY_MASK;
        out
    }
}

/// Complete zero-allocation stereo Tube Resonator FX stage.
pub struct TubeResonatorStereo {
    sample_rate: f64,
    filter_l: SofteningBandpass,
    filter_r: SofteningBandpass,
    res_l: WaveguideResonator,
    res_r: WaveguideResonator,
    dc_l: DcBlocker29,
    dc_r: DcBlocker29,
    lfo: PitchLfo,
    limiter_env: f64,
    last_lfo_phase_rad: f32,
    idle_samples: u32,
}

impl TubeResonatorStereo {
    pub fn new(sample_rate: f32) -> Self {
        let sr = (sample_rate as f64).max(8_000.0);
        Self {
            sample_rate: sr,
            filter_l: SofteningBandpass::default(),
            filter_r: SofteningBandpass::default(),
            res_l: WaveguideResonator::new(),
            res_r: WaveguideResonator::new(),
            dc_l: DcBlocker29::new(29.93, sr),
            dc_r: DcBlocker29::new(29.93, sr),
            lfo: PitchLfo::new(),
            limiter_env: 1.0,
            last_lfo_phase_rad: 0.0,
            idle_samples: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = (sample_rate as f64).max(8_000.0);
        self.sample_rate = sr;
        self.dc_l.set_cutoff(29.93, sr);
        self.dc_r.set_cutoff(29.93, sr);
        self.filter_l.last_sr = 0.0;
        self.filter_r.last_sr = 0.0;
    }

    pub fn reset(&mut self) {
        self.filter_l.reset();
        self.filter_r.reset();
        self.res_l.reset();
        self.res_r.reset();
        self.dc_l.reset();
        self.dc_r.reset();
        self.lfo.reset();
        self.limiter_env = 1.0;
        self.last_lfo_phase_rad = 0.0;
        self.idle_samples = 0;
    }

    #[inline]
    pub fn last_lfo_phase_rad(&self) -> f32 {
        self.last_lfo_phase_rad
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        filter_enabled: bool,
        dirty_lfo: bool,
    ) -> (f32, f32, f32) {
        let mix = p[3].clamp(0.0, 1.0) as f64;
        if mix <= 1e-4 && self.idle_samples > 12_000 {
            return (in_l, in_r, 0.0);
        }

        let sr = self.sample_rate;
        let base_tune_hz = tube_res_tune_hz(p[0]) as f64;
        let flt_freq_hz = tube_res_flt_freq_hz(p[1]) as f64;
        let flt_bw_oct = tube_res_flt_width_oct(p[2]) as f64;
        let lfo_amt = eq[0].clamp(0.0, 1.0) as f64;
        let lfo_rate_hz = tube_res_lfo_rate_hz(eq[1]) as f64;
        let decay_s = (tube_res_decay_ms(eq[2]) as f64) * 0.001;
        let damping = eq[3].clamp(0.0, 1.0) as f64;

        let lfo_still_running = lfo_amt > 1e-4;
        let idle_thresh = (sr * decay_s.max(0.25)) as u32;
        if track_idle(&mut self.idle_samples, in_l, in_r)
            && !lfo_still_running
            && self.idle_samples > idle_thresh
        {
            self.reset();
            return (in_l, in_r, 0.0);
        }

        if filter_enabled {
            self.filter_l.update(flt_freq_hz, flt_bw_oct, sr);
            self.filter_r.update(flt_freq_hz, flt_bw_oct, sr);
        }

        // Matches tube-resonator default Width = 0% (mono sum excitation into stereo waveguide)
        let dry_l = in_l as f64;
        let dry_r = in_r as f64;
        let mono_in = 0.5 * (dry_l + dry_r);

        let (exc_l, exc_r) = if filter_enabled {
            (self.filter_l.process(mono_in), self.filter_r.process(mono_in))
        } else {
            (mono_in, mono_in)
        };

        let (lfo_mul, phase_rad) = self.lfo.next_multiplier(lfo_amt, lfo_rate_hz, dirty_lfo, sr);
        self.last_lfo_phase_rad = phase_rad;

        let inst_freq = base_tune_hz * lfo_mul;
        let res_l = self.res_l.process_sample(exc_l, inst_freq, decay_s, damping, sr);
        let res_r = self.res_r.process_sample(exc_r, inst_freq, decay_s, damping, sr);

        let mut wet_l = self.dc_l.process(res_l);
        let mut wet_r = self.dc_r.process(res_r);

        let peak = wet_l.abs().max(wet_r.abs());
        let target_g = if peak > 1.0 { 1.0 / peak } else { 1.0 };
        let coeff = if target_g < self.limiter_env {
            0.2
        } else {
            0.0005
        };
        self.limiter_env += coeff * (target_g - self.limiter_env);
        wet_l = (wet_l * self.limiter_env).clamp(-1.0, 1.0);
        wet_r = (wet_r * self.limiter_env).clamp(-1.0, 1.0);

        let ol = ((1.0 - mix) * dry_l + mix * wet_l) as f32;
        let or = ((1.0 - mix) * dry_r + mix * wet_r) as f32;
        let act = wet_l.abs().max(wet_r.abs()).clamp(0.0, 1.0) as f32 * (mix as f32);
        (flush_denormal(ol), flush_denormal(or), act)
    }
}

/// Evaluates steady-state magnitude response (`0.0..=1.5`) at `freq_hz` for the `TUBE RES.`
/// live scope (harmonic comb resonances modulated by loop bandpass envelope).
pub fn evaluate_tube_res_scope_response(
    p: [f32; 4],
    eq: [f32; 4],
    filter_enabled: bool,
    lfo_phase_rad: f32,
    freq_hz: f32,
) -> (f32, f32) {
    let base_hz = tube_res_tune_hz(p[0]);
    let lfo_amt = eq[0].clamp(0.0, 1.0);
    let lfo_mul = (lfo_amt * lfo_phase_rad.sin()).exp2();
    let f0 = (base_hz * lfo_mul).clamp(15.0, 5_000.0);

    // Comb harmonic peaks at integer multiples of `f0`
    let ratio = (freq_hz / f0).max(0.1);
    let decay_ms = tube_res_decay_ms(eq[2]);
    let q_sharpness = 2.0 + ((decay_ms - 10.0) / 4_990.0).sqrt() * 28.0;
    let phase = (std::f32::consts::PI * ratio).sin().abs();
    let comb_mag = 1.0 / (1.0 + q_sharpness * phase * phase);

    // Bandpass envelope (`2nd-order Butterworth HP * LP`) when `FLT` enabled
    let bp_env = if filter_enabled {
        let fc = tube_res_flt_freq_hz(p[1]) * 1.015;
        let half_bw = 0.5 * tube_res_flt_width_oct(p[2]);
        let f_hp = (fc * (-half_bw).exp2()).clamp(20.0, 20_000.0);
        let f_lp = (fc * half_bw.exp2()).clamp(20.0, 20_000.0);
        let hp_mag = {
            let r = (freq_hz / f_hp).max(1e-4);
            let r2 = r * r;
            r2 / ((1.0 - r2).powi(2) + 2.0 * r2).sqrt()
        };
        let lp_mag = {
            let r = (freq_hz / f_lp).max(1e-4);
            let r2 = r * r;
            1.0 / ((1.0 - r2).powi(2) + 2.0 * r2).sqrt()
        };
        (hp_mag * lp_mag).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let mix = p[3].clamp(0.0, 1.0);
    let combined = ((1.0 - mix) * 0.25 + mix * comb_mag * bp_env).clamp(0.0, 1.0);
    (combined, bp_env)
}
