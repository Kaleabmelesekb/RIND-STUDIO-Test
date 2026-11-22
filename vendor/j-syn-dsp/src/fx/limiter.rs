use super::{flush_denormal, track_idle};
use crate::adaa::TanhAdaa2;
use crate::oversample::Oversampler4x;

const MAX_LOOKAHEAD_MS: f32 = 3.0;
const MAX_LOOKAHEAD_SAMPLES: usize = 640; // >= 3.0 ms at 192 kHz (576 samples)

/// Maps normalized `0.0..=1.0` to Limiter input gain `0.0..=+24.0 dB`.
#[inline]
pub fn limiter_gain_db(norm: f32) -> f32 {
    norm.clamp(0.0, 1.0) * 24.0
}

/// Maps normalized `0.0..=1.0` to Limiter ceiling `-18.0..=0.0 dB`.
#[inline]
pub fn limiter_ceil_db(norm: f32) -> f32 {
    -18.0 + norm.clamp(0.0, 1.0) * 18.0
}

/// Maps normalized `0.0..=1.0` to Limiter lookahead `0.0..=3.0 ms`.
#[inline]
pub fn limiter_lookahead_ms(norm: f32) -> f32 {
    norm.clamp(0.0, 1.0) * MAX_LOOKAHEAD_MS
}

/// Maps normalized `0.0..=1.0` to Limiter release base time `10.0..=500.0 ms`.
#[inline]
pub fn limiter_release_ms(norm: f32) -> f32 {
    10.0 * 50.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Fixed-capacity monotonic max-queue over ring buffer indices (zero heap allocations).
struct RingMaxDeque {
    ring: [f32; MAX_LOOKAHEAD_SAMPLES],
    deque_idx: [usize; MAX_LOOKAHEAD_SAMPLES],
    deque_pos: [u64; MAX_LOOKAHEAD_SAMPLES],
    head: usize,
    tail: usize,
    len: usize,
}

impl RingMaxDeque {
    fn new() -> Self {
        Self {
            ring: [0.0; MAX_LOOKAHEAD_SAMPLES],
            deque_idx: [0; MAX_LOOKAHEAD_SAMPLES],
            deque_pos: [0; MAX_LOOKAHEAD_SAMPLES],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }

    #[inline]
    fn push_and_max(&mut self, write_idx: usize, sample_pos: u64, val: f32, window_len: usize) -> f32 {
        self.ring[write_idx] = val;
        let w = window_len.max(1);
        let min_pos = sample_pos.saturating_sub((w - 1) as u64);

        while self.len > 0 && self.deque_pos[self.head] < min_pos {
            self.head = (self.head + 1) % MAX_LOOKAHEAD_SAMPLES;
            self.len -= 1;
        }

        while self.len > 0 {
            let back = (self.tail + MAX_LOOKAHEAD_SAMPLES - 1) % MAX_LOOKAHEAD_SAMPLES;
            if self.ring[self.deque_idx[back]] <= val {
                self.tail = back;
                self.len -= 1;
            } else {
                break;
            }
        }

        self.deque_idx[self.tail] = write_idx;
        self.deque_pos[self.tail] = sample_pos;
        self.tail = (self.tail + 1) % MAX_LOOKAHEAD_SAMPLES;
        self.len += 1;

        self.ring[self.deque_idx[self.head]]
    }
}

/// Zero-allocation hybrid Lookahead S-Curve + Zero-Lookahead Transient-Clipping Limiter.
pub(crate) struct LimiterStereo {
    delay_l: [f32; MAX_LOOKAHEAD_SAMPLES],
    delay_r: [f32; MAX_LOOKAHEAD_SAMPLES],
    dry_l: [f32; MAX_LOOKAHEAD_SAMPLES],
    dry_r: [f32; MAX_LOOKAHEAD_SAMPLES],
    write_idx: usize,
    sample_pos: u64,
    peak_deque_l: RingMaxDeque,
    peak_deque_r: RingMaxDeque,
    detect_os: [Oversampler4x; 2],
    clip_os: [Oversampler4x; 2],
    dry_os: [Oversampler4x; 2],
    adaa: [TanhAdaa2; 2],
    fast_env: [f32; 2],
    s_curve_env: [f32; 2],
    slow_env: f32,
    rms_sq: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl LimiterStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        Self {
            delay_l: [0.0; MAX_LOOKAHEAD_SAMPLES],
            delay_r: [0.0; MAX_LOOKAHEAD_SAMPLES],
            dry_l: [0.0; MAX_LOOKAHEAD_SAMPLES],
            dry_r: [0.0; MAX_LOOKAHEAD_SAMPLES],
            write_idx: 0,
            sample_pos: 0,
            peak_deque_l: RingMaxDeque::new(),
            peak_deque_r: RingMaxDeque::new(),
            detect_os: [Oversampler4x::new(); 2],
            clip_os: [Oversampler4x::new(); 2],
            dry_os: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            fast_env: [1.0; 2],
            s_curve_env: [1.0; 2],
            slow_env: 1.0,
            rms_sq: 0.0,
            sample_rate: sample_rate.max(1_000.0),
            idle_samples: 1024,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    pub(crate) fn reset(&mut self) {
        self.delay_l.fill(0.0);
        self.delay_r.fill(0.0);
        self.dry_l.fill(0.0);
        self.dry_r.fill(0.0);
        self.write_idx = 0;
        self.sample_pos = 0;
        self.peak_deque_l.reset();
        self.peak_deque_r.reset();
        for ch in 0..2 {
            self.detect_os[ch].reset();
            self.clip_os[ch].reset();
            self.dry_os[ch].reset();
            self.adaa[ch].reset();
        }
        self.fast_env = [1.0; 2];
        self.s_curve_env = [1.0; 2];
        self.slow_env = 1.0;
        self.rms_sq = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        true_peak: bool,
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 768 {
            return (0.0, 0.0, 0.0);
        }

        let gain_db = limiter_gain_db(p[0]);
        let gain_lin = 10.0_f32.powf(gain_db * 0.05);
        let ceil_db = limiter_ceil_db(p[1]);
        let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).clamp(0.05, 1.0);
        let lookahead_ms = limiter_lookahead_ms(p[2]);
        let mix = p[3].clamp(0.0, 1.0);

        let release_ms = limiter_release_ms(eq[0]);
        let knee = eq[1].clamp(0.0, 1.0);
        let link = eq[2].clamp(0.0, 1.0);
        let punch = eq[3].clamp(0.0, 1.0);

        let driven_l = in_l * gain_lin;
        let driven_r = in_r * gain_lin;

        let lookahead_samples = ((lookahead_ms * 0.001 * self.sample_rate).round() as usize)
            .min(MAX_LOOKAHEAD_SAMPLES - 1);

        self.delay_l[self.write_idx] = driven_l;
        self.delay_r[self.write_idx] = driven_r;
        self.dry_l[self.write_idx] = in_l;
        self.dry_r[self.write_idx] = in_r;

        // True-Peak 4x oversampled detection vs sample-peak detection
        let (det_l, det_r) = if true_peak {
            let mut pk_l = driven_l.abs();
            let _ = self.detect_os[0].process_sample(driven_l, |s| {
                pk_l = pk_l.max(s.abs());
                s
            });
            let mut pk_r = driven_r.abs();
            let _ = self.detect_os[1].process_sample(driven_r, |s| {
                pk_r = pk_r.max(s.abs());
                s
            });
            (pk_l, pk_r)
        } else {
            (driven_l.abs(), driven_r.abs())
        };

        let win_len = lookahead_samples + 1;
        let max_l = self
            .peak_deque_l
            .push_and_max(self.write_idx, self.sample_pos, det_l, win_len);
        let max_r = self
            .peak_deque_r
            .push_and_max(self.write_idx, self.sample_pos, det_r, win_len);

        let linked_max = max_l.max(max_r);
        let win_peaks = [
            max_l + (linked_max - max_l) * link,
            max_r + (linked_max - max_r) * link,
        ];

        // Dual-stage program-dependent release:
        // - Fast stage recovers quickly on brief percussive transients.
        // - Slow RMS stage prevents 40-60 Hz sub-bass cycle rippling (IMD).
        let rms_alpha = 1.0 - (-1.0 / (0.080 * self.sample_rate)).exp();
        let inst_sq = 0.5 * (driven_l * driven_l + driven_r * driven_r);
        self.rms_sq += rms_alpha * (inst_sq - self.rms_sq);
        let rms_peak = self.rms_sq.max(0.0).sqrt() * std::f32::consts::SQRT_2;
        let slow_req = (ceil_lin / rms_peak.max(1e-9)).min(1.0);

        let slow_att_alpha = 1.0 - (-1.0 / (0.030 * self.sample_rate)).exp();
        let slow_rel_s = (release_ms * 1.8 * 0.001).max(0.04);
        let slow_rel_alpha = 1.0 - (-1.0 / (slow_rel_s * self.sample_rate)).exp();
        let s_alpha = if slow_req < self.slow_env {
            slow_att_alpha
        } else {
            slow_rel_alpha
        };
        self.slow_env = (self.slow_env + s_alpha * (slow_req - self.slow_env)).clamp(0.01, 1.0);

        // Attack time constant:
        // - At 0 ms lookahead, ~1.5-5 ms non-instantaneous attack intentionally lets
        //   micro-transients hit the final safety clipper for punch without ducking sub-bass.
        // - At 1-3 ms lookahead, 2-stage smoothed attack (~0.32 * lookahead_s) completes
        //   smoothly right as the delayed peak arrives (C1 S-curve).
        let la_blend = (lookahead_ms / 1.0).clamp(0.0, 1.0);
        let zero_la_att_s = 0.0016 + punch * 0.0034;
        let la_att_s = (lookahead_ms * 0.001 * (0.30 + punch * 1.1)).max(0.00025);
        let att_s = zero_la_att_s + (la_att_s - zero_la_att_s) * la_blend;
        let att_alpha = 1.0 - (-1.0 / (att_s * self.sample_rate)).exp();

        let fast_rel_s = (release_ms * 0.18 * 0.001).max(0.004);
        let fast_rel_alpha = 1.0 - (-1.0 / (fast_rel_s * self.sample_rate)).exp();

        for (ch, &win_pk) in win_peaks.iter().enumerate() {
            let req = (ceil_lin / win_pk.max(1e-9)).min(1.0);
            let f_alpha = if req < self.fast_env[ch] {
                att_alpha
            } else {
                fast_rel_alpha
            };
            self.fast_env[ch] = (self.fast_env[ch] + f_alpha * (req - self.fast_env[ch])).clamp(0.01, 1.0);

            let combined = self.fast_env[ch].min(self.slow_env);
            let sc_alpha = if combined < self.s_curve_env[ch] {
                att_alpha
            } else {
                fast_rel_alpha
            };
            self.s_curve_env[ch] =
                (self.s_curve_env[ch] + sc_alpha * (combined - self.s_curve_env[ch])).clamp(0.01, 1.0);
        }

        let read_idx = (self.write_idx + MAX_LOOKAHEAD_SAMPLES - lookahead_samples) % MAX_LOOKAHEAD_SAMPLES;
        let dl = self.delay_l[read_idx];
        let dr = self.delay_r[read_idx];
        let dry_l = self.dry_l[read_idx];
        let dry_r = self.dry_r[read_idx];

        self.write_idx = (self.write_idx + 1) % MAX_LOOKAHEAD_SAMPLES;
        self.sample_pos = self.sample_pos.wrapping_add(1);

        let att_l = dl * self.s_curve_env[0];
        let att_r = dr * self.s_curve_env[1];

        let mut clipped = [0.0_f32; 2];
        let inv_ceil = 1.0 / ceil_lin;
        for (ch, &x) in [att_l, att_r].iter().enumerate() {
            let adaa = &mut self.adaa[ch];
            let os_out = self.clip_os[ch].process_sample(x, |s| {
                let hard = s.clamp(-ceil_lin, ceil_lin);
                let soft = ceil_lin * (adaa.process((s * inv_ceil) as f64) as f32);
                (hard + (soft - hard) * knee).clamp(-ceil_lin, ceil_lin)
            });
            // Guard against polyphase IIR downsampler Gibbs overshoot above ceiling.
            clipped[ch] = os_out.clamp(-ceil_lin, ceil_lin);
        }

        let (out_l, out_r) = if mix >= 0.999 {
            (clipped[0], clipped[1])
        } else {
            let dry_aligned_l = self.dry_os[0].process_sample(dry_l, |s| s);
            let dry_aligned_r = self.dry_os[1].process_sample(dry_r, |s| s);
            (
                dry_aligned_l + (clipped[0] - dry_aligned_l) * mix,
                dry_aligned_r + (clipped[1] - dry_aligned_r) * mix,
            )
        };

        let gr_env = 1.0 - self.s_curve_env[0].min(self.s_curve_env[1]);
        let pre_clip_pk = att_l.abs().max(att_r.abs());
        let clip_gr = if pre_clip_pk > ceil_lin {
            ((pre_clip_pk - ceil_lin) / pre_clip_pk.max(1e-6)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let activity = (gr_env.max(clip_gr) * mix).clamp(0.0, 1.0);

        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}
