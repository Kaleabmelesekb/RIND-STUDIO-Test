//! Hybrid 16-step Performer / MSEG modulator (`MODSEQ 1` – `MODSEQ 4`) with curve tension,
//! shape presets, DAW tempo sync, loop/one-shot/ping-pong modes, and slew smoothing.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

pub const MAX_MODSEQ_STEPS: usize = 16;
pub const MAX_MSEG_NODES: usize = 16;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqMode {
    #[default]
    Performer = 0,
    Mseg = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqStepShape {
    #[default]
    Linear = 0,
    Flat = 1,
    RampUp = 2,
    RampDown = 3,
    ExpUp = 4,
    ExpDown = 5,
    LogUp = 6,
    LogDown = 7,
    SineArchUp = 8,
    SineArchDown = 9,
    Triangle = 10,
    Pulse = 11,
    SCurve = 12,
    RampDown2x = 13,
    TriSine2x = 14,
    Square2x = 15,
    RampDown3x = 16,
    TriSine3x = 17,
    Square3x = 18,
}

impl ModSeqStepShape {
    /// Ordered 2x9 graphical palette tiles (Row 1: 9 single curves, Row 2: 3 single + 6 subdivided 2x/3x bursts).
    pub const PALETTE: [Self; 18] = [
        Self::Flat,
        Self::RampUp,
        Self::RampDown,
        Self::ExpUp,
        Self::ExpDown,
        Self::LogUp,
        Self::LogDown,
        Self::SineArchUp,
        Self::SineArchDown,
        Self::Triangle,
        Self::Pulse,
        Self::SCurve,
        Self::RampDown2x,
        Self::TriSine2x,
        Self::Square2x,
        Self::RampDown3x,
        Self::TriSine3x,
        Self::Square3x,
    ];

    pub const STAMPS: [Self; 18] = Self::PALETTE;

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Flat,
            2 => Self::RampUp,
            3 => Self::RampDown,
            4 => Self::ExpUp,
            5 => Self::ExpDown,
            6 => Self::LogUp,
            7 => Self::LogDown,
            8 => Self::SineArchUp,
            9 => Self::SineArchDown,
            10 => Self::Triangle,
            11 => Self::Pulse,
            12 => Self::SCurve,
            13 => Self::RampDown2x,
            14 => Self::TriSine2x,
            15 => Self::Square2x,
            16 => Self::RampDown3x,
            17 => Self::TriSine3x,
            18 => Self::Square3x,
            _ => Self::Linear,
        }
    }

    #[inline]
    pub fn subdivision(self) -> u8 {
        match self {
            Self::RampDown2x | Self::TriSine2x | Self::Square2x => 2,
            Self::RampDown3x | Self::TriSine3x | Self::Square3x => 3,
            _ => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "LIN",
            Self::Flat => "FLAT",
            Self::RampUp => "RAMP UP",
            Self::RampDown => "RAMP DN",
            Self::ExpUp => "EXP UP",
            Self::ExpDown => "EXP DN",
            Self::LogUp => "LOG UP",
            Self::LogDown => "LOG DN",
            Self::SineArchUp => "ARCH UP",
            Self::SineArchDown => "ARCH DN",
            Self::Triangle => "TRI",
            Self::Pulse => "SQUARE",
            Self::SCurve => "S-CURVE",
            Self::RampDown2x => "2x RAMP",
            Self::TriSine2x => "2x SINE",
            Self::Square2x => "2x SQR",
            Self::RampDown3x => "3x RAMP",
            Self::TriSine3x => "3x SINE",
            Self::Square3x => "3x SQR",
        }
    }
}

/// Cure Scream-style breakpoint preset templates for `LFO` (MSEG) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoPreset {
    Flat,
    RampUp,
    RampDown,
    ConcaveArc,
    ConvexArc,
    SCurve,
    Triangle,
    Sine,
    VShape,
}

impl LfoPreset {
    pub const ALL: [Self; 9] = [
        Self::Flat,
        Self::RampUp,
        Self::RampDown,
        Self::ConcaveArc,
        Self::ConvexArc,
        Self::SCurve,
        Self::Triangle,
        Self::Sine,
        Self::VShape,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "FLAT",
            Self::RampUp => "RAMP+",
            Self::RampDown => "RAMP-",
            Self::ConcaveArc => "CONCV",
            Self::ConvexArc => "CONVX",
            Self::SCurve => "S-CRV",
            Self::Triangle => "TRI",
            Self::Sine => "SINE",
            Self::VShape => "V-SHP",
        }
    }

    pub fn to_mseg(self) -> MsegCurve {
        let mut nodes = [MsegNode::default(); MAX_MSEG_NODES];
        let num_nodes = match self {
            Self::Flat => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.5, tension: 0.0 };
                nodes[1] = MsegNode { time_x: 1.0, level_y: 0.5, tension: 0.0 };
                2
            }
            Self::RampUp => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.0, tension: 0.0 };
                nodes[1] = MsegNode { time_x: 1.0, level_y: 1.0, tension: 0.0 };
                2
            }
            Self::RampDown => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 1.0, tension: 0.0 };
                nodes[1] = MsegNode { time_x: 1.0, level_y: 0.0, tension: 0.0 };
                2
            }
            Self::ConcaveArc => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.0, tension: 0.55 };
                nodes[1] = MsegNode { time_x: 1.0, level_y: 1.0, tension: 0.0 };
                2
            }
            Self::ConvexArc => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.0, tension: -0.55 };
                nodes[1] = MsegNode { time_x: 1.0, level_y: 1.0, tension: 0.0 };
                2
            }
            Self::SCurve => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.0, tension: 0.42 };
                nodes[1] = MsegNode { time_x: 0.5, level_y: 0.5, tension: -0.42 };
                nodes[2] = MsegNode { time_x: 1.0, level_y: 1.0, tension: 0.0 };
                3
            }
            Self::Triangle => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.0, tension: 0.0 };
                nodes[1] = MsegNode { time_x: 0.5, level_y: 1.0, tension: 0.0 };
                nodes[2] = MsegNode { time_x: 1.0, level_y: 0.0, tension: 0.0 };
                3
            }
            Self::Sine => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 0.5, tension: -0.35 };
                nodes[1] = MsegNode { time_x: 0.25, level_y: 1.0, tension: 0.33 };
                nodes[2] = MsegNode { time_x: 0.5, level_y: 0.5, tension: -0.35 };
                nodes[3] = MsegNode { time_x: 0.75, level_y: 0.0, tension: 0.33 };
                nodes[4] = MsegNode { time_x: 1.0, level_y: 0.5, tension: 0.0 };
                5
            }
            Self::VShape => {
                nodes[0] = MsegNode { time_x: 0.0, level_y: 1.0, tension: 0.0 };
                nodes[1] = MsegNode { time_x: 0.5, level_y: 0.0, tension: 0.0 };
                nodes[2] = MsegNode { time_x: 1.0, level_y: 1.0, tension: 0.0 };
                3
            }
        };
        MsegCurve { nodes, num_nodes }
    }
}

/// Single step segment in a `MODSEQ` Performer curve (`0.0..=1.0` levels, `-1.0..=1.0` tension).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModSeqStep {
    pub start_level: f32,
    pub end_level: f32,
    pub tension: f32,
    pub shape: ModSeqStepShape,
}

#[derive(Deserialize)]
#[serde(default)]
struct ModSeqStepWire {
    start_level: f32,
    end_level: f32,
    tension: f32,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    shape: Option<ModSeqStepShape>,
}

impl Default for ModSeqStepWire {
    fn default() -> Self {
        let d = ModSeqStep::default();
        Self {
            start_level: d.start_level,
            end_level: d.end_level,
            tension: d.tension,
            shape: Some(d.shape),
        }
    }
}

impl<'de> Deserialize<'de> for ModSeqStep {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSeqStepWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            start_level: crate::serde_lenient::finite_clamped(w.start_level, 0.5, 0.0, 1.0),
            end_level: crate::serde_lenient::finite_clamped(w.end_level, 0.5, 0.0, 1.0),
            tension: crate::serde_lenient::finite_clamped(w.tension, 0.0, -1.0, 1.0),
            shape: w.shape.unwrap_or(ModSeqStepShape::Linear),
        })
    }
}

impl Default for ModSeqStep {
    fn default() -> Self {
        Self {
            start_level: 0.5,
            end_level: 0.5,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        }
    }
}

impl ModSeqStep {
    #[inline]
    pub fn stamped(shape: ModSeqStepShape, depth: f32, tension: f32) -> Self {
        let d = depth.clamp(0.0, 1.0);
        let end = match shape {
            ModSeqStepShape::Flat
            | ModSeqStepShape::RampUp
            | ModSeqStepShape::ExpUp
            | ModSeqStepShape::LogUp
            | ModSeqStepShape::SineArchDown
            | ModSeqStepShape::SCurve
            | ModSeqStepShape::Linear => d,
            _ => 0.0,
        };
        Self {
            start_level: d,
            end_level: end,
            tension: tension.clamp(-1.0, 1.0),
            shape,
        }
    }
}

/// Movable breakpoint node in a continuous `MSEG` envelope (`time_x`, `level_y`, `tension`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MsegNode {
    pub time_x: f32,
    pub level_y: f32,
    pub tension: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MsegNodeWire {
    time_x: f32,
    level_y: f32,
    tension: f32,
}

impl<'de> Deserialize<'de> for MsegNode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<MsegNodeWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            time_x: crate::serde_lenient::finite_clamped(w.time_x, 0.0, 0.0, 1.0),
            level_y: crate::serde_lenient::finite_clamped(w.level_y, 0.0, 0.0, 1.0),
            tension: crate::serde_lenient::finite_clamped(w.tension, 0.0, -1.0, 1.0),
        })
    }
}

impl Default for MsegNode {
    fn default() -> Self {
        Self {
            time_x: 0.0,
            level_y: 0.0,
            tension: 0.0,
        }
    }
}

/// Fixed-size zero-allocation multi-segment envelope (`2..=16` nodes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MsegCurve {
    pub nodes: [MsegNode; MAX_MSEG_NODES],
    pub num_nodes: usize,
}

#[derive(Deserialize)]
#[serde(default)]
struct MsegCurveWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    nodes: [MsegNode; MAX_MSEG_NODES],
    num_nodes: usize,
}

impl Default for MsegCurveWire {
    fn default() -> Self {
        let d = MsegCurve::default();
        Self {
            nodes: d.nodes,
            num_nodes: d.num_nodes,
        }
    }
}

impl<'de> Deserialize<'de> for MsegCurve {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(mut w) = Option::<MsegCurveWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        if w.num_nodes < 2 {
            return Ok(Self::default());
        }
        let n = w.num_nodes.clamp(2, MAX_MSEG_NODES);
        w.nodes[0].time_x = 0.0;
        let mut prev_t = 0.0_f32;
        for node in &mut w.nodes[..n] {
            node.time_x = node.time_x.clamp(prev_t, 1.0);
            prev_t = node.time_x;
        }
        w.nodes[n - 1].time_x = 1.0;
        Ok(Self {
            nodes: w.nodes,
            num_nodes: n,
        })
    }
}

impl Default for MsegCurve {
    fn default() -> Self {
        let mut nodes = [MsegNode::default(); MAX_MSEG_NODES];
        nodes[0] = MsegNode {
            time_x: 0.0,
            level_y: 0.0,
            tension: -0.35,
        };
        nodes[1] = MsegNode {
            time_x: 0.15,
            level_y: 1.0,
            tension: 0.45,
        };
        nodes[2] = MsegNode {
            time_x: 0.55,
            level_y: 0.35,
            tension: 0.25,
        };
        nodes[3] = MsegNode {
            time_x: 1.0,
            level_y: 0.0,
            tension: 0.0,
        };
        Self {
            nodes,
            num_nodes: 4,
        }
    }
}

/// Fixed-size persistence/ArcSwap container holding Performer steps and MSEG curves for all 4 `MODSEQ` engines.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModSeqStepBank {
    pub seqs: [[ModSeqStep; MAX_MODSEQ_STEPS]; 4],
    pub msegs: [MsegCurve; 4],
}

#[derive(Deserialize)]
#[serde(transparent)]
struct ModSeqRowWire(
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")] [ModSeqStep; MAX_MODSEQ_STEPS],
);

impl Default for ModSeqRowWire {
    fn default() -> Self {
        Self([ModSeqStep::default(); MAX_MODSEQ_STEPS])
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ModSeqStepBankWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    seqs: [ModSeqRowWire; 4],
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    msegs: [MsegCurve; 4],
}

impl Default for ModSeqStepBankWire {
    fn default() -> Self {
        let d = ModSeqStepBank::default();
        Self {
            seqs: std::array::from_fn(|i| ModSeqRowWire(d.seqs[i])),
            msegs: d.msegs,
        }
    }
}

impl<'de> Deserialize<'de> for ModSeqStepBank {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSeqStepBankWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            seqs: std::array::from_fn(|i| w.seqs[i].0),
            msegs: w.msegs,
        })
    }
}

impl Default for ModSeqStepBank {
    fn default() -> Self {
        let mut s0 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s1 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s2 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s3 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];

        // MODSEQ 1: Expressive 8-step rhythmic pluck/ramp (zero swing tension by default)
        ModSeqShapePreset::RampDown.apply_to_sequence(&mut s0, 8);
        s0[0] = ModSeqStep {
            start_level: 1.0,
            end_level: 0.15,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        };
        s0[2] = ModSeqStep {
            start_level: 0.75,
            end_level: 0.1,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        };
        s0[4] = ModSeqStep {
            start_level: 0.9,
            end_level: 0.25,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        };
        s0[6] = ModSeqStep {
            start_level: 0.65,
            end_level: 0.0,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        };

        // MODSEQ 2: Smooth triangle wave
        ModSeqShapePreset::Triangle.apply_to_sequence(&mut s1, 8);
        // MODSEQ 3: Smooth sine wave
        ModSeqShapePreset::Sine.apply_to_sequence(&mut s2, 8);
        // MODSEQ 4: Stepped sample & hold groove
        ModSeqShapePreset::SampleAndHold.apply_to_sequence(&mut s3, 8);

        Self {
            seqs: [s0, s1, s2, s3],
            msegs: std::array::from_fn(|_| MsegCurve::default()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqShapePreset {
    RampUp,
    RampDown,
    Pulse,
    Triangle,
    Sine,
    SampleAndHold,
}

impl ModSeqShapePreset {
    pub const ALL: [Self; 6] = [
        Self::RampUp,
        Self::RampDown,
        Self::Pulse,
        Self::Triangle,
        Self::Sine,
        Self::SampleAndHold,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RampUp => "RAMP +",
            Self::RampDown => "RAMP -",
            Self::Pulse => "PULSE",
            Self::Triangle => "TRI",
            Self::Sine => "SINE",
            Self::SampleAndHold => "S&H",
        }
    }

    pub fn apply_to_sequence(self, steps: &mut [ModSeqStep; MAX_MODSEQ_STEPS], num_steps: usize) {
        let n = num_steps.clamp(2, MAX_MODSEQ_STEPS);
        for (i, step) in steps.iter_mut().take(MAX_MODSEQ_STEPS).enumerate() {
            let idx = i % n;
            let t0 = idx as f32 / n as f32;
            let t1 = (idx + 1) as f32 / n as f32;
            *step = match self {
                Self::RampUp => ModSeqStep {
                    start_level: t0,
                    end_level: t1,
                    tension: 0.0,
                    shape: ModSeqStepShape::Linear,
                },
                Self::RampDown => ModSeqStep {
                    start_level: 1.0 - t0,
                    end_level: 1.0 - t1,
                    tension: 0.0,
                    shape: ModSeqStepShape::Linear,
                },
                Self::Pulse => {
                    let lvl = if idx < n / 2 { 1.0 } else { 0.0 };
                    ModSeqStep {
                        start_level: lvl,
                        end_level: lvl,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                    }
                }
                Self::Triangle => {
                    let tri = |t: f32| {
                        if t < 0.5 {
                            t * 2.0
                        } else {
                            (1.0 - t) * 2.0
                        }
                    };
                    ModSeqStep {
                        start_level: tri(t0),
                        end_level: tri(t1),
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                    }
                }
                Self::Sine => {
                    let s = |t: f32| 0.5 - 0.5 * (2.0 * PI * t).cos();
                    ModSeqStep {
                        start_level: s(t0),
                        end_level: s(t1),
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                    }
                }
                Self::SampleAndHold => {
                    let v = ((idx.wrapping_mul(73) + 19) % 13) as f32 / 12.0;
                    ModSeqStep {
                        start_level: v,
                        end_level: v,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                    }
                }
            };
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqSyncMode {
    Free = 0,
    Straight = 1,
    Triplet = 2,
    Dotted = 3,
}

impl ModSeqSyncMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Free,
            2 => Self::Triplet,
            3 => Self::Dotted,
            _ => Self::Straight,
        }
    }
}

pub const MODSEQ_STRAIGHT_DIVS: [(&str, f32); 10] = [
    ("8 Bars", 1.0 / 32.0),
    ("4 Bars", 1.0 / 16.0),
    ("2 Bars", 1.0 / 8.0),
    ("1 Bar", 1.0 / 4.0),
    ("1/2", 0.5),
    ("1/4", 1.0),
    ("1/8", 2.0),
    ("1/16", 4.0),
    ("1/32", 8.0),
    ("1/64", 16.0),
];

pub const MODSEQ_TRIPLET_DIVS: [(&str, f32); 5] = [
    ("1/2T", 0.75),
    ("1/4T", 1.5),
    ("1/8T", 3.0),
    ("1/16T", 6.0),
    ("1/32T", 12.0),
];

pub const MODSEQ_DOTTED_DIVS: [(&str, f32); 5] = [
    ("1/2D", 1.0 / 3.0),
    ("1/4D", 2.0 / 3.0),
    ("1/8D", 4.0 / 3.0),
    ("1/16D", 8.0 / 3.0),
    ("1/32D", 16.0 / 3.0),
];

#[inline]
fn sync_divs(sync_mode: ModSeqSyncMode) -> &'static [(&'static str, f32)] {
    match sync_mode {
        ModSeqSyncMode::Free => &[],
        ModSeqSyncMode::Straight => &MODSEQ_STRAIGHT_DIVS,
        ModSeqSyncMode::Triplet => &MODSEQ_TRIPLET_DIVS,
        ModSeqSyncMode::Dotted => &MODSEQ_DOTTED_DIVS,
    }
}

/// Maps normalized `0.0..=1.0` rate knob position to continuous `0.05..=50.0 Hz` in `Free` mode.
#[inline]
pub fn modseq_free_norm_to_hz(norm: f32) -> f32 {
    (0.05 * 1000.0_f32.powf(norm.clamp(0.0, 1.0))).clamp(0.05, 50.0)
}

/// Maps normalized `0.0..=1.0` rate knob position to the snapped division label for sync modes.
#[inline]
pub fn modseq_division_label(sync_mode: ModSeqSyncMode, rate_norm: f32) -> &'static str {
    let divs = sync_divs(sync_mode);
    if divs.is_empty() {
        return "Free";
    }
    let idx = ((rate_norm.clamp(0.0, 1.0) * (divs.len() as f32 - 0.001)).floor() as usize)
        .min(divs.len() - 1);
    divs[idx].0
}

/// Computes effective modulator frequency in Hz for any `ModSeqSyncMode` and normalized rate.
#[inline]
pub fn modseq_rate_hz(sync_mode: ModSeqSyncMode, rate_norm: f32, tempo_bpm: f32) -> f32 {
    let divs = sync_divs(sync_mode);
    if divs.is_empty() {
        modseq_free_norm_to_hz(rate_norm)
    } else {
        let idx = ((rate_norm.clamp(0.0, 1.0) * (divs.len() as f32 - 0.001)).floor() as usize)
            .min(divs.len() - 1);
        (tempo_bpm.clamp(20.0, 300.0) / 60.0) * divs[idx].1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqPlayMode {
    Loop = 0,
    OneShot = 1,
    PingPong = 2,
}

impl ModSeqPlayMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::OneShot,
            2 => Self::PingPong,
            _ => Self::Loop,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqRetrig {
    PolyVoice = 0,
    MonoGlobal = 1,
}

impl ModSeqRetrig {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::MonoGlobal,
            _ => Self::PolyVoice,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModSeqParams {
    pub mode: ModSeqMode,
    pub sync_mode: ModSeqSyncMode,
    /// Unified normalized `0.0..=1.0` rate knob position (continuous `0.05..=50.0 Hz` in `Free`,
    /// snapped across musical divisions in `Straight` / `Triplet` / `Dotted`).
    pub rate_norm: f32,
    pub play_mode: ModSeqPlayMode,
    pub retrig: ModSeqRetrig,
    pub num_steps: usize,
    pub smooth: f32,
    pub rate_mod: f32,
    pub groove_swing: bool,
}

impl Default for ModSeqParams {
    fn default() -> Self {
        Self {
            mode: ModSeqMode::Performer,
            sync_mode: ModSeqSyncMode::Straight,
            rate_norm: 0.35, // 1 Bar in Straight mode
            play_mode: ModSeqPlayMode::Loop,
            retrig: ModSeqRetrig::PolyVoice,
            num_steps: 8,
            smooth: 0.05,
            rate_mod: 0.0,
            groove_swing: false,
        }
    }
}

/// Evaluates a single `ModSeqStep` at normalized step-local phase `local_t` in `[0.0, 1.0]`. Zero allocations.
#[inline]
pub fn evaluate_step_shape(step: ModSeqStep, local_t: f32) -> f32 {
    let t = local_t.clamp(0.0, 1.0);
    let d = step.start_level.clamp(0.0, 1.0);
    let exp = 2.0_f32.powf(step.tension.clamp(-1.0, 1.0) * 2.2);
    let ct = t.powf(exp);

    match step.shape {
        ModSeqStepShape::Linear => {
            (step.start_level + (step.end_level - step.start_level) * ct).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Flat => d,
        ModSeqStepShape::RampUp => (d * ct).clamp(0.0, 1.0),
        ModSeqStepShape::RampDown => (d * (1.0 - ct)).clamp(0.0, 1.0),
        ModSeqStepShape::ExpUp => (d * t.powf(2.6 * exp)).clamp(0.0, 1.0),
        ModSeqStepShape::ExpDown => (d * (1.0 - t).powf(2.6 * exp)).clamp(0.0, 1.0),
        ModSeqStepShape::LogUp => {
            let p = (2.6 / exp).clamp(0.15, 16.0);
            (d * (1.0 - (1.0 - t).powf(p))).clamp(0.0, 1.0)
        }
        ModSeqStepShape::LogDown => {
            let p = (2.6 / exp).clamp(0.15, 16.0);
            (d * (1.0 - t.powf(p))).clamp(0.0, 1.0)
        }
        ModSeqStepShape::SineArchUp => (d * (PI * ct).sin().clamp(0.0, 1.0)).clamp(0.0, 1.0),
        ModSeqStepShape::SineArchDown => (d * (1.0 - (PI * ct).sin().clamp(0.0, 1.0))).clamp(0.0, 1.0),
        ModSeqStepShape::Triangle => {
            let tri = if ct < 0.5 { ct * 2.0 } else { (1.0 - ct) * 2.0 };
            (d * tri).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Pulse => {
            if ct < 0.5 {
                d
            } else {
                0.0
            }
        }
        ModSeqStepShape::SCurve => (d * (0.5 - 0.5 * (PI * ct).cos())).clamp(0.0, 1.0),
        ModSeqStepShape::RampDown2x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 2.0).fract().powf(exp);
            (d * (1.0 - sub_ct)).clamp(0.0, 1.0)
        }
        ModSeqStepShape::TriSine2x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 2.0).fract().powf(exp);
            (d * (0.5 - 0.5 * (2.0 * PI * sub_ct).cos())).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Square2x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 2.0).fract().powf(exp);
            if sub_ct < 0.5 {
                d
            } else {
                0.0
            }
        }
        ModSeqStepShape::RampDown3x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 3.0).fract().powf(exp);
            (d * (1.0 - sub_ct)).clamp(0.0, 1.0)
        }
        ModSeqStepShape::TriSine3x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 3.0).fract().powf(exp);
            (d * (0.5 - 0.5 * (2.0 * PI * sub_ct).cos())).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Square3x => {
            let sub_ct = (local_t.clamp(0.0, 0.999_999) * 3.0).fract().powf(exp);
            if sub_ct < 0.5 {
                d
            } else {
                0.0
            }
        }
    }
}

/// Experimental alternating step-phase swing tension pattern (`[-0.40, 0.0, -0.35, 0.0, ...]`).
const GROOVE_SWING_TENSIONS: [f32; 8] = [-0.40, 0.0, -0.35, 0.0, -0.30, 0.0, -0.50, 0.0];

/// Evaluates the shaped Performer step curve with optional experimental alternating groove swing.
#[inline]
pub fn evaluate_modseq_curve_with_swing(
    steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    num_steps: usize,
    phase: f32,
    groove_swing: bool,
) -> f32 {
    let n = num_steps.clamp(2, MAX_MODSEQ_STEPS);
    let scaled = phase.clamp(0.0, 0.999_999) * (n as f32);
    let step_idx = (scaled.floor() as usize).min(n - 1);
    let local_t = (scaled - (step_idx as f32)).clamp(0.0, 1.0);
    let mut step = steps[step_idx];
    if groove_swing {
        let swing_t = GROOVE_SWING_TENSIONS[step_idx % GROOVE_SWING_TENSIONS.len()];
        step.tension = (step.tension + swing_t).clamp(-1.0, 1.0);
    }
    evaluate_step_shape(step, local_t)
}

/// Evaluates the shaped Performer step curve at normalized cycle phase `phase` in `[0.0, 1.0)`.
#[inline]
pub fn evaluate_modseq_curve(
    steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    num_steps: usize,
    phase: f32,
) -> f32 {
    evaluate_modseq_curve_with_swing(steps, num_steps, phase, false)
}

/// Evaluates a continuous multi-segment `MSEG` curve at normalized phase in `[0.0, 1.0]`.
#[inline]
pub fn evaluate_mseg_curve(mseg: &MsegCurve, phase: f32) -> f32 {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let p = phase.clamp(0.0, 1.0);
    if p <= mseg.nodes[0].time_x {
        return mseg.nodes[0].level_y.clamp(0.0, 1.0);
    }
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if p <= b.time_x || i + 2 == n {
            let span = (b.time_x - a.time_x).max(1e-5);
            let local_t = ((p - a.time_x) / span).clamp(0.0, 1.0);
            let exp = 2.0_f32.powf(a.tension.clamp(-1.0, 1.0) * 2.5);
            let curved_t = local_t.powf(exp);
            return (a.level_y + (b.level_y - a.level_y) * curved_t).clamp(0.0, 1.0);
        }
    }
    mseg.nodes[n - 1].level_y.clamp(0.0, 1.0)
}

/// Per-voice (or global) runtime state for one `MODSEQ` unit. Zero allocations.
#[derive(Debug, Clone, Copy)]
pub struct ModSeqState {
    pub phase: f32,
    pub direction: f32,
    pub completed: bool,
    pub smoothed_out: f32,
}

impl Default for ModSeqState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModSeqState {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            direction: 1.0,
            completed: false,
            smoothed_out: 0.0,
        }
    }

    #[inline]
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
        self.smoothed_out = 0.0;
    }

    /// Advances phase by `dt_seconds` and returns the slew-smoothed Performer or MSEG value in `[0.0, 1.0]`.
    #[inline]
    pub fn advance_with_mseg(
        &mut self,
        dt_seconds: f32,
        tempo_bpm: f32,
        params: &ModSeqParams,
        steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
        mseg: &MsegCurve,
    ) -> f32 {
        let eff_norm = (params.rate_norm + params.rate_mod).clamp(0.0, 1.0);
        let freq_hz = modseq_rate_hz(params.sync_mode, eff_norm, tempo_bpm).clamp(0.01, 100.0);
        let d_phase = freq_hz * dt_seconds.max(0.0);

        match params.play_mode {
            ModSeqPlayMode::Loop => {
                self.completed = false;
                self.direction = 1.0;
                self.phase = (self.phase + d_phase).rem_euclid(1.0);
            }
            ModSeqPlayMode::OneShot => {
                if !self.completed {
                    self.phase += d_phase;
                    if self.phase >= 1.0 {
                        self.phase = 0.999_999;
                        self.completed = true;
                    }
                }
            }
            ModSeqPlayMode::PingPong => {
                self.completed = false;
                self.phase += d_phase * self.direction;
                if self.phase >= 1.0 {
                    self.phase = (2.0 - self.phase).clamp(0.0, 0.999_999);
                    self.direction = -1.0;
                } else if self.phase <= 0.0 {
                    self.phase = (-self.phase).clamp(0.0, 0.999_999);
                    self.direction = 1.0;
                }
            }
        }

        let raw = match params.mode {
            ModSeqMode::Performer => evaluate_modseq_curve_with_swing(
                steps,
                params.num_steps,
                self.phase,
                params.groove_swing,
            ),
            ModSeqMode::Mseg => evaluate_mseg_curve(mseg, self.phase),
        };
        let smooth = params.smooth.clamp(0.0, 1.0);
        if smooth <= 1e-4 {
            self.smoothed_out = raw;
        } else {
            let tau_s = 0.0005 + smooth * smooth * 0.060;
            let coeff = (1.0 - (-dt_seconds.max(1e-6) / tau_s).exp()).clamp(0.0, 1.0);
            self.smoothed_out += (raw - self.smoothed_out) * coeff;
        }
        self.smoothed_out
    }

    #[inline]
    pub fn advance(
        &mut self,
        dt_seconds: f32,
        tempo_bpm: f32,
        params: &ModSeqParams,
        steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    ) -> f32 {
        self.advance_with_mseg(dt_seconds, tempo_bpm, params, steps, &MsegCurve::default())
    }
}
