//! Lock-free 32-slot Modulation Matrix and real-time UI telemetry primitives.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

pub const NUM_MOD_SLOTS: usize = 32;
pub const NUM_ACTIVE_SOURCES: usize = 21;
pub const NUM_MOD_DESTS: usize = 272;

/// Modulation sources assignable in any matrix slot.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModMatrixSource {
    #[default]
    None = 0,
    Env1 = 1,
    Env2 = 2,
    Env3 = 3,
    Env4 = 4,
    ModSeq1 = 5,
    ModSeq2 = 6,
    ModSeq3 = 7,
    ModSeq4 = 8,
    Velocity = 9,
    Keytrack = 10,
    ModWheel = 11,
    Aftertouch = 12,
    Macro1 = 13,
    Macro2 = 14,
    Macro3 = 15,
    Macro4 = 16,
    Macro5 = 17,
    Macro6 = 18,
    Macro7 = 19,
    Macro8 = 20,
    Slide = 21,
}

impl ModMatrixSource {
    pub const ALL_ACTIVE: [ModMatrixSource; NUM_ACTIVE_SOURCES] = [
        Self::Env1,
        Self::Env2,
        Self::Env3,
        Self::Env4,
        Self::ModSeq1,
        Self::ModSeq2,
        Self::ModSeq3,
        Self::ModSeq4,
        Self::Velocity,
        Self::Keytrack,
        Self::ModWheel,
        Self::Aftertouch,
        Self::Slide,
        Self::Macro1,
        Self::Macro2,
        Self::Macro3,
        Self::Macro4,
        Self::Macro5,
        Self::Macro6,
        Self::Macro7,
        Self::Macro8,
    ];

    pub const MACROS: [ModMatrixSource; 8] = [
        Self::Macro1,
        Self::Macro2,
        Self::Macro3,
        Self::Macro4,
        Self::Macro5,
        Self::Macro6,
        Self::Macro7,
        Self::Macro8,
    ];

    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Env1,
            2 => Self::Env2,
            3 => Self::Env3,
            4 => Self::Env4,
            5 => Self::ModSeq1,
            6 => Self::ModSeq2,
            7 => Self::ModSeq3,
            8 => Self::ModSeq4,
            9 => Self::Velocity,
            10 => Self::Keytrack,
            11 => Self::ModWheel,
            12 => Self::Aftertouch,
            13 => Self::Macro1,
            14 => Self::Macro2,
            15 => Self::Macro3,
            16 => Self::Macro4,
            17 => Self::Macro5,
            18 => Self::Macro6,
            19 => Self::Macro7,
            20 => Self::Macro8,
            21 => Self::Slide,
            _ => Self::None,
        }
    }

    #[inline]
    pub fn active_index(self) -> Option<usize> {
        let raw = self as u8;
        if raw >= 1 && (raw as usize) <= NUM_ACTIVE_SOURCES {
            Some((raw as usize) - 1)
        } else {
            None
        }
    }

    #[inline]
    pub fn short_label(self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Env1 => "E1",
            Self::Env2 => "E2",
            Self::Env3 => "E3",
            Self::Env4 => "E4",
            Self::ModSeq1 => "MS1",
            Self::ModSeq2 => "MS2",
            Self::ModSeq3 => "MS3",
            Self::ModSeq4 => "MS4",
            Self::Velocity => "VEL",
            Self::Keytrack => "KEY",
            Self::ModWheel => "MW",
            Self::Aftertouch => "AT",
            Self::Slide => "SLD",
            Self::Macro1 => "M1",
            Self::Macro2 => "M2",
            Self::Macro3 => "M3",
            Self::Macro4 => "M4",
            Self::Macro5 => "M5",
            Self::Macro6 => "M6",
            Self::Macro7 => "M7",
            Self::Macro8 => "M8",
        }
    }

    #[inline]
    pub fn full_label(self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Env1 => "ENV 1 (AMP)",
            Self::Env2 => "ENV 2",
            Self::Env3 => "ENV 3",
            Self::Env4 => "ENV 4",
            Self::ModSeq1 => "MODSEQ 1",
            Self::ModSeq2 => "MODSEQ 2",
            Self::ModSeq3 => "MODSEQ 3",
            Self::ModSeq4 => "MODSEQ 4",
            Self::Velocity => "VELOCITY",
            Self::Keytrack => "KEYTRACK",
            Self::ModWheel => "MOD WHEEL",
            Self::Aftertouch => "AFTERTOUCH",
            Self::Slide => "SLIDE (CC74)",
            Self::Macro1 => "MACRO 1",
            Self::Macro2 => "MACRO 2",
            Self::Macro3 => "MACRO 3",
            Self::Macro4 => "MACRO 4",
            Self::Macro5 => "MACRO 5",
            Self::Macro6 => "MACRO 6",
            Self::Macro7 => "MACRO 7",
            Self::Macro8 => "MACRO 8",
        }
    }

    #[inline]
    pub fn color_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::None => (140, 125, 110),
            // Envelopes: Mint (#00E676) high-contrast family
            Self::Env1 => (0, 230, 118),
            Self::Env2 => (46, 242, 150),
            Self::Env3 => (0, 212, 142),
            Self::Env4 => (92, 248, 172),
            // ModSeqs: Cyan (#00E5FF) high-contrast family
            Self::ModSeq1 => (0, 229, 255),
            Self::ModSeq2 => (56, 206, 255),
            Self::ModSeq3 => (0, 245, 232),
            Self::ModSeq4 => (100, 220, 255),
            // MIDI / performance sources: Coral (#FF5252) high-contrast family
            Self::Velocity => (255, 82, 82),
            Self::Keytrack => (255, 112, 96),
            Self::ModWheel => (255, 92, 130),
            Self::Aftertouch => (255, 126, 82),
            Self::Slide => (255, 152, 108),
            // Macros: Warm Amber/Violet high-contrast family
            Self::Macro1 => (255, 200, 72),
            Self::Macro2 => (255, 178, 60),
            Self::Macro3 => (245, 152, 66),
            Self::Macro4 => (255, 214, 102),
            Self::Macro5 => (222, 148, 255),
            Self::Macro6 => (196, 136, 255),
            Self::Macro7 => (240, 128, 224),
            Self::Macro8 => (176, 158, 255),
        }
    }
}

/// All modulatable continuous & pitch destinations across `J-SYN`.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModDestination {
    #[default]
    Osc1WtPos = 0,
    Osc2WtPos = 1,
    Osc3WtPos = 2,
    Osc4WtPos = 3,
    Osc1Warp = 4,
    Osc2Warp = 5,
    Osc3Warp = 6,
    Osc4Warp = 7,
    Osc1ModDepth = 8,
    Osc2ModDepth = 9,
    Osc3ModDepth = 10,
    Osc4ModDepth = 11,
    Osc1Pan = 12,
    Osc2Pan = 13,
    Osc3Pan = 14,
    Osc4Pan = 15,
    Osc1Level = 16,
    Osc2Level = 17,
    Osc3Level = 18,
    Osc4Level = 19,
    Osc1Pitch = 20,
    Osc2Pitch = 21,
    Osc3Pitch = 22,
    Osc4Pitch = 23,
    Osc1Phase = 24,
    Osc2Phase = 25,
    Osc3Phase = 26,
    Osc4Phase = 27,
    Osc1Detune = 28,
    Osc2Detune = 29,
    Osc3Detune = 30,
    Osc4Detune = 31,
    Osc1Spread = 32,
    Osc2Spread = 33,
    Osc3Spread = 34,
    Osc4Spread = 35,
    NoiseColor = 36,
    NoiseStereo = 37,
    NoiseLevel = 38,
    Filter1Cutoff = 39,
    Filter1Res = 40,
    Filter1Drive = 41,
    Filter1Aux = 42,
    Filter2Cutoff = 43,
    Filter2Res = 44,
    Filter2Drive = 45,
    Filter2Aux = 46,
    Insert1Drive = 47,
    Insert1Mix = 48,
    Insert2Drive = 49,
    Insert2Mix = 50,
    Osc1FilterSend = 51,
    Osc2FilterSend = 52,
    Osc3FilterSend = 53,
    Osc4FilterSend = 54,
    NoiseFilterSend = 55,
    FilterMix = 56,
    FeedbackAmount = 57,
    GlideMs = 58,
    ModSeq1Rate = 59,
    ModSeq2Rate = 60,
    ModSeq3Rate = 61,
    ModSeq4Rate = 62,
    Fx1P1 = 63,
    Fx1P2 = 64,
    Fx1P3 = 65,
    Fx1P4 = 66,
    Fx2P1 = 67,
    Fx2P2 = 68,
    Fx2P3 = 69,
    Fx2P4 = 70,
    Fx3P1 = 71,
    Fx3P2 = 72,
    Fx3P3 = 73,
    Fx3P4 = 74,
    Fx4P1 = 75,
    Fx4P2 = 76,
    Fx4P3 = 77,
    Fx4P4 = 78,
    Fx5P1 = 79,
    Fx5P2 = 80,
    Fx5P3 = 81,
    Fx5P4 = 82,
    Fx6P1 = 83,
    Fx6P2 = 84,
    Fx6P3 = 85,
    Fx6P4 = 86,
    Fx7P1 = 87,
    Fx7P2 = 88,
    Fx7P3 = 89,
    Fx7P4 = 90,
    Fx8P1 = 91,
    Fx8P2 = 92,
    Fx8P3 = 93,
    Fx8P4 = 94,
    // Bus B main FX params P1..P4 (95..=126)
    BusBFx1P1 = 95,
    BusBFx1P2 = 96,
    BusBFx1P3 = 97,
    BusBFx1P4 = 98,
    BusBFx2P1 = 99,
    BusBFx2P2 = 100,
    BusBFx2P3 = 101,
    BusBFx2P4 = 102,
    BusBFx3P1 = 103,
    BusBFx3P2 = 104,
    BusBFx3P3 = 105,
    BusBFx3P4 = 106,
    BusBFx4P1 = 107,
    BusBFx4P2 = 108,
    BusBFx4P3 = 109,
    BusBFx4P4 = 110,
    BusBFx5P1 = 111,
    BusBFx5P2 = 112,
    BusBFx5P3 = 113,
    BusBFx5P4 = 114,
    BusBFx6P1 = 115,
    BusBFx6P2 = 116,
    BusBFx6P3 = 117,
    BusBFx6P4 = 118,
    BusBFx7P1 = 119,
    BusBFx7P2 = 120,
    BusBFx7P3 = 121,
    BusBFx7P4 = 122,
    BusBFx8P1 = 123,
    BusBFx8P2 = 124,
    BusBFx8P3 = 125,
    BusBFx8P4 = 126,
    // Bus A extended/EQ FX params P5..P8 (127..=158)
    BusAFx1P5 = 127,
    BusAFx1P6 = 128,
    BusAFx1P7 = 129,
    BusAFx1P8 = 130,
    BusAFx2P5 = 131,
    BusAFx2P6 = 132,
    BusAFx2P7 = 133,
    BusAFx2P8 = 134,
    BusAFx3P5 = 135,
    BusAFx3P6 = 136,
    BusAFx3P7 = 137,
    BusAFx3P8 = 138,
    BusAFx4P5 = 139,
    BusAFx4P6 = 140,
    BusAFx4P7 = 141,
    BusAFx4P8 = 142,
    BusAFx5P5 = 143,
    BusAFx5P6 = 144,
    BusAFx5P7 = 145,
    BusAFx5P8 = 146,
    BusAFx6P5 = 147,
    BusAFx6P6 = 148,
    BusAFx6P7 = 149,
    BusAFx6P8 = 150,
    BusAFx7P5 = 151,
    BusAFx7P6 = 152,
    BusAFx7P7 = 153,
    BusAFx7P8 = 154,
    BusAFx8P5 = 155,
    BusAFx8P6 = 156,
    BusAFx8P7 = 157,
    BusAFx8P8 = 158,
    // Bus B extended/EQ FX params P5..P8 (159..=190)
    BusBFx1P5 = 159,
    BusBFx1P6 = 160,
    BusBFx1P7 = 161,
    BusBFx1P8 = 162,
    BusBFx2P5 = 163,
    BusBFx2P6 = 164,
    BusBFx2P7 = 165,
    BusBFx2P8 = 166,
    BusBFx3P5 = 167,
    BusBFx3P6 = 168,
    BusBFx3P7 = 169,
    BusBFx3P8 = 170,
    BusBFx4P5 = 171,
    BusBFx4P6 = 172,
    BusBFx4P7 = 173,
    BusBFx4P8 = 174,
    BusBFx5P5 = 175,
    BusBFx5P6 = 176,
    BusBFx5P7 = 177,
    BusBFx5P8 = 178,
    BusBFx6P5 = 179,
    BusBFx6P6 = 180,
    BusBFx6P7 = 181,
    BusBFx6P8 = 182,
    BusBFx7P5 = 183,
    BusBFx7P6 = 184,
    BusBFx7P7 = 185,
    BusBFx7P8 = 186,
    BusBFx8P5 = 187,
    BusBFx8P6 = 188,
    BusBFx8P7 = 189,
    BusBFx8P8 = 190,
    // Sub Bus & Parallel Bus Mixer (191..=197)
    SubCutoff = 191,
    SubDrive = 192,
    SubLevel = 193,
    BusALevel = 194,
    BusAPan = 195,
    BusBLevel = 196,
    BusBPan = 197,
    // Oscillator Ratio & Linear Hz Offset (198..=205)
    Osc1Ratio = 198,
    Osc2Ratio = 199,
    Osc3Ratio = 200,
    Osc4Ratio = 201,
    Osc1HzOffset = 202,
    Osc2HzOffset = 203,
    Osc3HzOffset = 204,
    Osc4HzOffset = 205,
    // Bus C main FX params P1..P4 (206..=237)
    BusCFx1P1 = 206,
    BusCFx1P2 = 207,
    BusCFx1P3 = 208,
    BusCFx1P4 = 209,
    BusCFx2P1 = 210,
    BusCFx2P2 = 211,
    BusCFx2P3 = 212,
    BusCFx2P4 = 213,
    BusCFx3P1 = 214,
    BusCFx3P2 = 215,
    BusCFx3P3 = 216,
    BusCFx3P4 = 217,
    BusCFx4P1 = 218,
    BusCFx4P2 = 219,
    BusCFx4P3 = 220,
    BusCFx4P4 = 221,
    BusCFx5P1 = 222,
    BusCFx5P2 = 223,
    BusCFx5P3 = 224,
    BusCFx5P4 = 225,
    BusCFx6P1 = 226,
    BusCFx6P2 = 227,
    BusCFx6P3 = 228,
    BusCFx6P4 = 229,
    BusCFx7P1 = 230,
    BusCFx7P2 = 231,
    BusCFx7P3 = 232,
    BusCFx7P4 = 233,
    BusCFx8P1 = 234,
    BusCFx8P2 = 235,
    BusCFx8P3 = 236,
    BusCFx8P4 = 237,
    // Bus C extended/EQ FX params P5..P8 (238..=269)
    BusCFx1P5 = 238,
    BusCFx1P6 = 239,
    BusCFx1P7 = 240,
    BusCFx1P8 = 241,
    BusCFx2P5 = 242,
    BusCFx2P6 = 243,
    BusCFx2P7 = 244,
    BusCFx2P8 = 245,
    BusCFx3P5 = 246,
    BusCFx3P6 = 247,
    BusCFx3P7 = 248,
    BusCFx3P8 = 249,
    BusCFx4P5 = 250,
    BusCFx4P6 = 251,
    BusCFx4P7 = 252,
    BusCFx4P8 = 253,
    BusCFx5P5 = 254,
    BusCFx5P6 = 255,
    BusCFx5P7 = 256,
    BusCFx5P8 = 257,
    BusCFx6P5 = 258,
    BusCFx6P6 = 259,
    BusCFx6P7 = 260,
    BusCFx6P8 = 261,
    BusCFx7P5 = 262,
    BusCFx7P6 = 263,
    BusCFx7P7 = 264,
    BusCFx7P8 = 265,
    BusCFx8P5 = 266,
    BusCFx8P6 = 267,
    BusCFx8P7 = 268,
    BusCFx8P8 = 269,
    BusCLevel = 270,
    BusCPan = 271,
}

pub const ALL_MOD_DESTINATIONS: [ModDestination; NUM_MOD_DESTS] = [
    ModDestination::Osc1WtPos,
    ModDestination::Osc2WtPos,
    ModDestination::Osc3WtPos,
    ModDestination::Osc4WtPos,
    ModDestination::Osc1Warp,
    ModDestination::Osc2Warp,
    ModDestination::Osc3Warp,
    ModDestination::Osc4Warp,
    ModDestination::Osc1ModDepth,
    ModDestination::Osc2ModDepth,
    ModDestination::Osc3ModDepth,
    ModDestination::Osc4ModDepth,
    ModDestination::Osc1Pan,
    ModDestination::Osc2Pan,
    ModDestination::Osc3Pan,
    ModDestination::Osc4Pan,
    ModDestination::Osc1Level,
    ModDestination::Osc2Level,
    ModDestination::Osc3Level,
    ModDestination::Osc4Level,
    ModDestination::Osc1Pitch,
    ModDestination::Osc2Pitch,
    ModDestination::Osc3Pitch,
    ModDestination::Osc4Pitch,
    ModDestination::Osc1Phase,
    ModDestination::Osc2Phase,
    ModDestination::Osc3Phase,
    ModDestination::Osc4Phase,
    ModDestination::Osc1Detune,
    ModDestination::Osc2Detune,
    ModDestination::Osc3Detune,
    ModDestination::Osc4Detune,
    ModDestination::Osc1Spread,
    ModDestination::Osc2Spread,
    ModDestination::Osc3Spread,
    ModDestination::Osc4Spread,
    ModDestination::NoiseColor,
    ModDestination::NoiseStereo,
    ModDestination::NoiseLevel,
    ModDestination::Filter1Cutoff,
    ModDestination::Filter1Res,
    ModDestination::Filter1Drive,
    ModDestination::Filter1Aux,
    ModDestination::Filter2Cutoff,
    ModDestination::Filter2Res,
    ModDestination::Filter2Drive,
    ModDestination::Filter2Aux,
    ModDestination::Insert1Drive,
    ModDestination::Insert1Mix,
    ModDestination::Insert2Drive,
    ModDestination::Insert2Mix,
    ModDestination::Osc1FilterSend,
    ModDestination::Osc2FilterSend,
    ModDestination::Osc3FilterSend,
    ModDestination::Osc4FilterSend,
    ModDestination::NoiseFilterSend,
    ModDestination::FilterMix,
    ModDestination::FeedbackAmount,
    ModDestination::GlideMs,
    ModDestination::ModSeq1Rate,
    ModDestination::ModSeq2Rate,
    ModDestination::ModSeq3Rate,
    ModDestination::ModSeq4Rate,
    ModDestination::Fx1P1,
    ModDestination::Fx1P2,
    ModDestination::Fx1P3,
    ModDestination::Fx1P4,
    ModDestination::Fx2P1,
    ModDestination::Fx2P2,
    ModDestination::Fx2P3,
    ModDestination::Fx2P4,
    ModDestination::Fx3P1,
    ModDestination::Fx3P2,
    ModDestination::Fx3P3,
    ModDestination::Fx3P4,
    ModDestination::Fx4P1,
    ModDestination::Fx4P2,
    ModDestination::Fx4P3,
    ModDestination::Fx4P4,
    ModDestination::Fx5P1,
    ModDestination::Fx5P2,
    ModDestination::Fx5P3,
    ModDestination::Fx5P4,
    ModDestination::Fx6P1,
    ModDestination::Fx6P2,
    ModDestination::Fx6P3,
    ModDestination::Fx6P4,
    ModDestination::Fx7P1,
    ModDestination::Fx7P2,
    ModDestination::Fx7P3,
    ModDestination::Fx7P4,
    ModDestination::Fx8P1,
    ModDestination::Fx8P2,
    ModDestination::Fx8P3,
    ModDestination::Fx8P4,
    ModDestination::BusBFx1P1,
    ModDestination::BusBFx1P2,
    ModDestination::BusBFx1P3,
    ModDestination::BusBFx1P4,
    ModDestination::BusBFx2P1,
    ModDestination::BusBFx2P2,
    ModDestination::BusBFx2P3,
    ModDestination::BusBFx2P4,
    ModDestination::BusBFx3P1,
    ModDestination::BusBFx3P2,
    ModDestination::BusBFx3P3,
    ModDestination::BusBFx3P4,
    ModDestination::BusBFx4P1,
    ModDestination::BusBFx4P2,
    ModDestination::BusBFx4P3,
    ModDestination::BusBFx4P4,
    ModDestination::BusBFx5P1,
    ModDestination::BusBFx5P2,
    ModDestination::BusBFx5P3,
    ModDestination::BusBFx5P4,
    ModDestination::BusBFx6P1,
    ModDestination::BusBFx6P2,
    ModDestination::BusBFx6P3,
    ModDestination::BusBFx6P4,
    ModDestination::BusBFx7P1,
    ModDestination::BusBFx7P2,
    ModDestination::BusBFx7P3,
    ModDestination::BusBFx7P4,
    ModDestination::BusBFx8P1,
    ModDestination::BusBFx8P2,
    ModDestination::BusBFx8P3,
    ModDestination::BusBFx8P4,
    ModDestination::BusAFx1P5,
    ModDestination::BusAFx1P6,
    ModDestination::BusAFx1P7,
    ModDestination::BusAFx1P8,
    ModDestination::BusAFx2P5,
    ModDestination::BusAFx2P6,
    ModDestination::BusAFx2P7,
    ModDestination::BusAFx2P8,
    ModDestination::BusAFx3P5,
    ModDestination::BusAFx3P6,
    ModDestination::BusAFx3P7,
    ModDestination::BusAFx3P8,
    ModDestination::BusAFx4P5,
    ModDestination::BusAFx4P6,
    ModDestination::BusAFx4P7,
    ModDestination::BusAFx4P8,
    ModDestination::BusAFx5P5,
    ModDestination::BusAFx5P6,
    ModDestination::BusAFx5P7,
    ModDestination::BusAFx5P8,
    ModDestination::BusAFx6P5,
    ModDestination::BusAFx6P6,
    ModDestination::BusAFx6P7,
    ModDestination::BusAFx6P8,
    ModDestination::BusAFx7P5,
    ModDestination::BusAFx7P6,
    ModDestination::BusAFx7P7,
    ModDestination::BusAFx7P8,
    ModDestination::BusAFx8P5,
    ModDestination::BusAFx8P6,
    ModDestination::BusAFx8P7,
    ModDestination::BusAFx8P8,
    ModDestination::BusBFx1P5,
    ModDestination::BusBFx1P6,
    ModDestination::BusBFx1P7,
    ModDestination::BusBFx1P8,
    ModDestination::BusBFx2P5,
    ModDestination::BusBFx2P6,
    ModDestination::BusBFx2P7,
    ModDestination::BusBFx2P8,
    ModDestination::BusBFx3P5,
    ModDestination::BusBFx3P6,
    ModDestination::BusBFx3P7,
    ModDestination::BusBFx3P8,
    ModDestination::BusBFx4P5,
    ModDestination::BusBFx4P6,
    ModDestination::BusBFx4P7,
    ModDestination::BusBFx4P8,
    ModDestination::BusBFx5P5,
    ModDestination::BusBFx5P6,
    ModDestination::BusBFx5P7,
    ModDestination::BusBFx5P8,
    ModDestination::BusBFx6P5,
    ModDestination::BusBFx6P6,
    ModDestination::BusBFx6P7,
    ModDestination::BusBFx6P8,
    ModDestination::BusBFx7P5,
    ModDestination::BusBFx7P6,
    ModDestination::BusBFx7P7,
    ModDestination::BusBFx7P8,
    ModDestination::BusBFx8P5,
    ModDestination::BusBFx8P6,
    ModDestination::BusBFx8P7,
    ModDestination::BusBFx8P8,
    ModDestination::SubCutoff,
    ModDestination::SubDrive,
    ModDestination::SubLevel,
    ModDestination::BusALevel,
    ModDestination::BusAPan,
    ModDestination::BusBLevel,
    ModDestination::BusBPan,
    ModDestination::Osc1Ratio,
    ModDestination::Osc2Ratio,
    ModDestination::Osc3Ratio,
    ModDestination::Osc4Ratio,
    ModDestination::Osc1HzOffset,
    ModDestination::Osc2HzOffset,
    ModDestination::Osc3HzOffset,
    ModDestination::Osc4HzOffset,
    ModDestination::BusCFx1P1,
    ModDestination::BusCFx1P2,
    ModDestination::BusCFx1P3,
    ModDestination::BusCFx1P4,
    ModDestination::BusCFx2P1,
    ModDestination::BusCFx2P2,
    ModDestination::BusCFx2P3,
    ModDestination::BusCFx2P4,
    ModDestination::BusCFx3P1,
    ModDestination::BusCFx3P2,
    ModDestination::BusCFx3P3,
    ModDestination::BusCFx3P4,
    ModDestination::BusCFx4P1,
    ModDestination::BusCFx4P2,
    ModDestination::BusCFx4P3,
    ModDestination::BusCFx4P4,
    ModDestination::BusCFx5P1,
    ModDestination::BusCFx5P2,
    ModDestination::BusCFx5P3,
    ModDestination::BusCFx5P4,
    ModDestination::BusCFx6P1,
    ModDestination::BusCFx6P2,
    ModDestination::BusCFx6P3,
    ModDestination::BusCFx6P4,
    ModDestination::BusCFx7P1,
    ModDestination::BusCFx7P2,
    ModDestination::BusCFx7P3,
    ModDestination::BusCFx7P4,
    ModDestination::BusCFx8P1,
    ModDestination::BusCFx8P2,
    ModDestination::BusCFx8P3,
    ModDestination::BusCFx8P4,
    ModDestination::BusCFx1P5,
    ModDestination::BusCFx1P6,
    ModDestination::BusCFx1P7,
    ModDestination::BusCFx1P8,
    ModDestination::BusCFx2P5,
    ModDestination::BusCFx2P6,
    ModDestination::BusCFx2P7,
    ModDestination::BusCFx2P8,
    ModDestination::BusCFx3P5,
    ModDestination::BusCFx3P6,
    ModDestination::BusCFx3P7,
    ModDestination::BusCFx3P8,
    ModDestination::BusCFx4P5,
    ModDestination::BusCFx4P6,
    ModDestination::BusCFx4P7,
    ModDestination::BusCFx4P8,
    ModDestination::BusCFx5P5,
    ModDestination::BusCFx5P6,
    ModDestination::BusCFx5P7,
    ModDestination::BusCFx5P8,
    ModDestination::BusCFx6P5,
    ModDestination::BusCFx6P6,
    ModDestination::BusCFx6P7,
    ModDestination::BusCFx6P8,
    ModDestination::BusCFx7P5,
    ModDestination::BusCFx7P6,
    ModDestination::BusCFx7P7,
    ModDestination::BusCFx7P8,
    ModDestination::BusCFx8P5,
    ModDestination::BusCFx8P6,
    ModDestination::BusCFx8P7,
    ModDestination::BusCFx8P8,
    ModDestination::BusCLevel,
    ModDestination::BusCPan,
];

const BUS_C_FX_MOD_DEST_NAMES: [&str; 64] = [
    "C FX1 P1", "C FX1 P2", "C FX1 P3", "C FX1 P4",
    "C FX2 P1", "C FX2 P2", "C FX2 P3", "C FX2 P4",
    "C FX3 P1", "C FX3 P2", "C FX3 P3", "C FX3 P4",
    "C FX4 P1", "C FX4 P2", "C FX4 P3", "C FX4 P4",
    "C FX5 P1", "C FX5 P2", "C FX5 P3", "C FX5 P4",
    "C FX6 P1", "C FX6 P2", "C FX6 P3", "C FX6 P4",
    "C FX7 P1", "C FX7 P2", "C FX7 P3", "C FX7 P4",
    "C FX8 P1", "C FX8 P2", "C FX8 P3", "C FX8 P4",
    "C FX1 P5", "C FX1 P6", "C FX1 P7", "C FX1 P8",
    "C FX2 P5", "C FX2 P6", "C FX2 P7", "C FX2 P8",
    "C FX3 P5", "C FX3 P6", "C FX3 P7", "C FX3 P8",
    "C FX4 P5", "C FX4 P6", "C FX4 P7", "C FX4 P8",
    "C FX5 P5", "C FX5 P6", "C FX5 P7", "C FX5 P8",
    "C FX6 P5", "C FX6 P6", "C FX6 P7", "C FX6 P8",
    "C FX7 P5", "C FX7 P6", "C FX7 P7", "C FX7 P8",
    "C FX8 P5", "C FX8 P6", "C FX8 P7", "C FX8 P8",
];

const FX_MOD_DEST_NAMES: [&str; 128] = [
    // Bus A P1..P4 (0..32)
    "A FX1 P1", "A FX1 P2", "A FX1 P3", "A FX1 P4",
    "A FX2 P1", "A FX2 P2", "A FX2 P3", "A FX2 P4",
    "A FX3 P1", "A FX3 P2", "A FX3 P3", "A FX3 P4",
    "A FX4 P1", "A FX4 P2", "A FX4 P3", "A FX4 P4",
    "A FX5 P1", "A FX5 P2", "A FX5 P3", "A FX5 P4",
    "A FX6 P1", "A FX6 P2", "A FX6 P3", "A FX6 P4",
    "A FX7 P1", "A FX7 P2", "A FX7 P3", "A FX7 P4",
    "A FX8 P1", "A FX8 P2", "A FX8 P3", "A FX8 P4",
    // Bus B P1..P4 (32..64)
    "B FX1 P1", "B FX1 P2", "B FX1 P3", "B FX1 P4",
    "B FX2 P1", "B FX2 P2", "B FX2 P3", "B FX2 P4",
    "B FX3 P1", "B FX3 P2", "B FX3 P3", "B FX3 P4",
    "B FX4 P1", "B FX4 P2", "B FX4 P3", "B FX4 P4",
    "B FX5 P1", "B FX5 P2", "B FX5 P3", "B FX5 P4",
    "B FX6 P1", "B FX6 P2", "B FX6 P3", "B FX6 P4",
    "B FX7 P1", "B FX7 P2", "B FX7 P3", "B FX7 P4",
    "B FX8 P1", "B FX8 P2", "B FX8 P3", "B FX8 P4",
    // Bus A P5..P8 (64..96)
    "A FX1 P5", "A FX1 P6", "A FX1 P7", "A FX1 P8",
    "A FX2 P5", "A FX2 P6", "A FX2 P7", "A FX2 P8",
    "A FX3 P5", "A FX3 P6", "A FX3 P7", "A FX3 P8",
    "A FX4 P5", "A FX4 P6", "A FX4 P7", "A FX4 P8",
    "A FX5 P5", "A FX5 P6", "A FX5 P7", "A FX5 P8",
    "A FX6 P5", "A FX6 P6", "A FX6 P7", "A FX6 P8",
    "A FX7 P5", "A FX7 P6", "A FX7 P7", "A FX7 P8",
    "A FX8 P5", "A FX8 P6", "A FX8 P7", "A FX8 P8",
    // Bus B P5..P8 (96..128)
    "B FX1 P5", "B FX1 P6", "B FX1 P7", "B FX1 P8",
    "B FX2 P5", "B FX2 P6", "B FX2 P7", "B FX2 P8",
    "B FX3 P5", "B FX3 P6", "B FX3 P7", "B FX3 P8",
    "B FX4 P5", "B FX4 P6", "B FX4 P7", "B FX4 P8",
    "B FX5 P5", "B FX5 P6", "B FX5 P7", "B FX5 P8",
    "B FX6 P5", "B FX6 P6", "B FX6 P7", "B FX6 P8",
    "B FX7 P5", "B FX7 P6", "B FX7 P7", "B FX7 P8",
    "B FX8 P5", "B FX8 P6", "B FX8 P7", "B FX8 P8",
];

impl ModDestination {
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(idx: usize) -> Option<Self> {
        if idx < NUM_MOD_DESTS {
            Some(ALL_MOD_DESTINATIONS[idx])
        } else {
            None
        }
    }

    #[inline]
    pub const fn osc_wt_pos(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1WtPos,
            1 => Self::Osc2WtPos,
            2 => Self::Osc3WtPos,
            _ => Self::Osc4WtPos,
        }
    }

    #[inline]
    pub const fn osc_warp(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Warp,
            1 => Self::Osc2Warp,
            2 => Self::Osc3Warp,
            _ => Self::Osc4Warp,
        }
    }

    #[inline]
    pub const fn osc_mod_depth(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1ModDepth,
            1 => Self::Osc2ModDepth,
            2 => Self::Osc3ModDepth,
            _ => Self::Osc4ModDepth,
        }
    }

    #[inline]
    pub const fn osc_pan(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Pan,
            1 => Self::Osc2Pan,
            2 => Self::Osc3Pan,
            _ => Self::Osc4Pan,
        }
    }

    #[inline]
    pub const fn osc_level(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Level,
            1 => Self::Osc2Level,
            2 => Self::Osc3Level,
            _ => Self::Osc4Level,
        }
    }

    #[inline]
    pub const fn osc_pitch(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Pitch,
            1 => Self::Osc2Pitch,
            2 => Self::Osc3Pitch,
            _ => Self::Osc4Pitch,
        }
    }

    #[inline]
    pub const fn osc_ratio(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Ratio,
            1 => Self::Osc2Ratio,
            2 => Self::Osc3Ratio,
            _ => Self::Osc4Ratio,
        }
    }

    #[inline]
    pub const fn osc_hz_offset(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1HzOffset,
            1 => Self::Osc2HzOffset,
            2 => Self::Osc3HzOffset,
            _ => Self::Osc4HzOffset,
        }
    }

    #[inline]
    pub const fn osc_phase(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Phase,
            1 => Self::Osc2Phase,
            2 => Self::Osc3Phase,
            _ => Self::Osc4Phase,
        }
    }

    #[inline]
    pub const fn osc_detune(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Detune,
            1 => Self::Osc2Detune,
            2 => Self::Osc3Detune,
            _ => Self::Osc4Detune,
        }
    }

    #[inline]
    pub const fn osc_spread(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Spread,
            1 => Self::Osc2Spread,
            2 => Self::Osc3Spread,
            _ => Self::Osc4Spread,
        }
    }

    #[inline]
    pub const fn filter_cutoff(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Cutoff
        } else {
            Self::Filter2Cutoff
        }
    }

    #[inline]
    pub const fn filter_res(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Res
        } else {
            Self::Filter2Res
        }
    }

    #[inline]
    pub const fn filter_drive(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Drive
        } else {
            Self::Filter2Drive
        }
    }

    #[inline]
    pub const fn filter_aux(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Aux
        } else {
            Self::Filter2Aux
        }
    }

    #[inline]
    pub const fn insert_drive(idx: usize) -> Self {
        if idx == 0 {
            Self::Insert1Drive
        } else {
            Self::Insert2Drive
        }
    }

    #[inline]
    pub const fn insert_mix(idx: usize) -> Self {
        if idx == 0 {
            Self::Insert1Mix
        } else {
            Self::Insert2Mix
        }
    }

    #[inline]
    pub const fn osc_filter_send(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1FilterSend,
            1 => Self::Osc2FilterSend,
            2 => Self::Osc3FilterSend,
            _ => Self::Osc4FilterSend,
        }
    }

    #[inline]
    pub const fn modseq_rate(idx: usize) -> Self {
        match idx {
            0 => Self::ModSeq1Rate,
            1 => Self::ModSeq2Rate,
            2 => Self::ModSeq3Rate,
            _ => Self::ModSeq4Rate,
        }
    }

    /// Returns destination for `bus` (`0` = Bus A, `1` = Bus B, `2` = Bus C), `slot` (`0..8`), and `param` (`0..8`).
    #[inline]
    pub const fn fx_bus_slot_param(bus: usize, slot: usize, param: usize) -> Self {
        let s = if slot < 8 { slot } else { 7 };
        let p = if param < 8 { param } else { 7 };
        let idx = if bus >= 2 {
            if p < 4 {
                206 + s * 4 + p
            } else {
                238 + s * 4 + (p - 4)
            }
        } else if p < 4 {
            63 + bus * 32 + s * 4 + p
        } else {
            127 + bus * 32 + s * 4 + (p - 4)
        };
        ALL_MOD_DESTINATIONS[idx]
    }

    #[inline]
    pub const fn fx_param(slot: usize, param: usize) -> Self {
        Self::fx_bus_slot_param(0, slot, param)
    }

    #[inline]
    pub const fn fx_slot_param_coords(self) -> Option<(usize, usize, usize)> {
        let idx = self as usize;
        if idx >= 63 && idx < 127 {
            let rel = idx - 63;
            Some((rel / 32, (rel % 32) / 4, rel % 4))
        } else if idx >= 127 && idx < 191 {
            let rel = idx - 127;
            Some((rel / 32, (rel % 32) / 4, 4 + (rel % 4)))
        } else if idx >= 206 && idx < 238 {
            let rel = idx - 206;
            Some((2, rel / 4, rel % 4))
        } else if idx >= 238 && idx < 270 {
            let rel = idx - 238;
            Some((2, rel / 4, 4 + (rel % 4)))
        } else {
            None
        }
    }

    #[inline]
    pub const fn is_fx_bus_slot(self, bus: usize, slot: usize) -> bool {
        match self.fx_slot_param_coords() {
            Some((b, s, _)) => b == bus && s == slot,
            None => false,
        }
    }

    #[inline]
    pub const fn is_fx_slot(self, slot: usize) -> bool {
        self.is_fx_bus_slot(0, slot)
    }

    pub fn name(self) -> &'static str {
        let idx = self as usize;
        if idx >= 63 && idx < 191 {
            return FX_MOD_DEST_NAMES[idx - 63];
        }
        if idx >= 206 && idx < 270 {
            return BUS_C_FX_MOD_DEST_NAMES[idx - 206];
        }
        match self {
            Self::Osc1WtPos => "OSC1 WT",
            Self::Osc2WtPos => "OSC2 WT",
            Self::Osc3WtPos => "OSC3 WT",
            Self::Osc4WtPos => "OSC4 WT",
            Self::Osc1Warp => "OSC1 WARP",
            Self::Osc2Warp => "OSC2 WARP",
            Self::Osc3Warp => "OSC3 WARP",
            Self::Osc4Warp => "OSC4 WARP",
            Self::Osc1ModDepth => "OSC1 MOD",
            Self::Osc2ModDepth => "OSC2 MOD",
            Self::Osc3ModDepth => "OSC3 MOD",
            Self::Osc4ModDepth => "OSC4 MOD",
            Self::Osc1Pan => "OSC1 PAN",
            Self::Osc2Pan => "OSC2 PAN",
            Self::Osc3Pan => "OSC3 PAN",
            Self::Osc4Pan => "OSC4 PAN",
            Self::Osc1Level => "OSC1 LVL",
            Self::Osc2Level => "OSC2 LVL",
            Self::Osc3Level => "OSC3 LVL",
            Self::Osc4Level => "OSC4 LVL",
            Self::Osc1Pitch => "OSC1 PITCH",
            Self::Osc2Pitch => "OSC2 PITCH",
            Self::Osc3Pitch => "OSC3 PITCH",
            Self::Osc4Pitch => "OSC4 PITCH",
            Self::Osc1Phase => "OSC1 PHS",
            Self::Osc2Phase => "OSC2 PHS",
            Self::Osc3Phase => "OSC3 PHS",
            Self::Osc4Phase => "OSC4 PHS",
            Self::Osc1Detune => "OSC1 DET",
            Self::Osc2Detune => "OSC2 DET",
            Self::Osc3Detune => "OSC3 DET",
            Self::Osc4Detune => "OSC4 DET",
            Self::Osc1Spread => "OSC1 SPR",
            Self::Osc2Spread => "OSC2 SPR",
            Self::Osc3Spread => "OSC3 SPR",
            Self::Osc4Spread => "OSC4 SPR",
            Self::NoiseColor => "NSE CLR",
            Self::NoiseStereo => "NSE ST",
            Self::NoiseLevel => "NSE LVL",
            Self::Filter1Cutoff => "F1 CUT",
            Self::Filter1Res => "F1 RES",
            Self::Filter1Drive => "F1 DRV",
            Self::Filter1Aux => "F1 AUX",
            Self::Filter2Cutoff => "F2 CUT",
            Self::Filter2Res => "F2 RES",
            Self::Filter2Drive => "F2 DRV",
            Self::Filter2Aux => "F2 AUX",
            Self::Insert1Drive => "INS1 DRV",
            Self::Insert1Mix => "INS1 MIX",
            Self::Insert2Drive => "INS2 DRV",
            Self::Insert2Mix => "INS2 MIX",
            Self::Osc1FilterSend => "O1 SEND",
            Self::Osc2FilterSend => "O2 SEND",
            Self::Osc3FilterSend => "O3 SEND",
            Self::Osc4FilterSend => "O4 SEND",
            Self::NoiseFilterSend => "NS SEND",
            Self::FilterMix => "F1/2 MIX",
            Self::FeedbackAmount => "FEEDBACK",
            Self::GlideMs => "GLIDE",
            Self::ModSeq1Rate => "MS1 RATE",
            Self::ModSeq2Rate => "MS2 RATE",
            Self::ModSeq3Rate => "MS3 RATE",
            Self::ModSeq4Rate => "MS4 RATE",
            Self::SubCutoff => "SUB CUT",
            Self::SubDrive => "SUB DRV",
            Self::SubLevel => "SUB LVL",
            Self::BusALevel => "BUS A LVL",
            Self::BusAPan => "BUS A PAN",
            Self::BusBLevel => "BUS B LVL",
            Self::BusBPan => "BUS B PAN",
            Self::BusCLevel => "BUS C LVL",
            Self::BusCPan => "BUS C PAN",
            Self::Osc1Ratio => "OSC1 RATIO",
            Self::Osc2Ratio => "OSC2 RATIO",
            Self::Osc3Ratio => "OSC3 RATIO",
            Self::Osc4Ratio => "OSC4 RATIO",
            Self::Osc1HzOffset => "OSC1 HZ",
            Self::Osc2HzOffset => "OSC2 HZ",
            Self::Osc3HzOffset => "OSC3 HZ",
            Self::Osc4HzOffset => "OSC4 HZ",
            _ => "",
        }
    }
}

/// Single modulation routing slot inside the fixed 32-slot matrix.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct ModSlot {
    pub source: ModMatrixSource,
    pub destination: ModDestination,
    pub depth: f32,
    pub bipolar: bool,
    pub enabled: bool,
    pub bypassed: bool,
    pub aux_source: ModMatrixSource,
    pub curve: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ModSlotWire {
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    source: Option<ModMatrixSource>,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    destination: Option<ModDestination>,
    depth: f32,
    bipolar: bool,
    enabled: bool,
    bypassed: bool,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    aux_source: Option<ModMatrixSource>,
    curve: f32,
}

impl<'de> Deserialize<'de> for ModSlot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSlotWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let (source, destination, mut enabled) = match (w.source, w.destination) {
            (Some(src), Some(dst)) => (src, dst, w.enabled),
            (Some(src), None) => (src, ModDestination::default(), false),
            (None, dst) => (ModMatrixSource::None, dst.unwrap_or_default(), false),
        };
        if source == ModMatrixSource::None {
            enabled = false;
        }
        Ok(Self {
            source,
            destination,
            depth: crate::serde_lenient::finite_clamped(w.depth, 0.0, -1.0, 1.0),
            bipolar: w.bipolar,
            enabled,
            bypassed: w.bypassed,
            aux_source: w.aux_source.unwrap_or(ModMatrixSource::None),
            curve: crate::serde_lenient::finite_clamped(w.curve, 0.0, -1.0, 3.0),
        })
    }
}

/// Pre-allocated fixed-size 32-slot modulation matrix (`Copy`, lock-free swap & persistence).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModMatrixState {
    pub slots: [ModSlot; NUM_MOD_SLOTS],
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ModMatrixStateWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    slots: [ModSlot; NUM_MOD_SLOTS],
}

impl<'de> Deserialize<'de> for ModMatrixState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModMatrixStateWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self { slots: w.slots })
    }
}

impl Default for ModMatrixState {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); NUM_MOD_SLOTS],
        }
    }
}

impl ModMatrixState {
    /// Finds an active slot matching `(source, destination)` or allocates the first free slot.
    pub fn assign_or_update(
        &mut self,
        source: ModMatrixSource,
        destination: ModDestination,
        default_depth: f32,
    ) -> Option<usize> {
        if source == ModMatrixSource::None {
            return None;
        }
        if let Some(idx) = self
            .slots
            .iter()
            .position(|s| s.enabled && s.source == source && s.destination == destination)
        {
            return Some(idx);
        }
        if let Some(free_idx) = self
            .slots
            .iter()
            .position(|s| !s.enabled || s.source == ModMatrixSource::None)
        {
            self.slots[free_idx] = ModSlot {
                source,
                destination,
                depth: default_depth.clamp(-1.0, 1.0),
                bipolar: false,
                enabled: true,
                bypassed: false,
                aux_source: ModMatrixSource::None,
                curve: 0.0,
            };
            return Some(free_idx);
        }
        None
    }

    pub fn clear_slot(&mut self, slot_idx: usize) {
        if slot_idx < NUM_MOD_SLOTS {
            self.slots[slot_idx] = ModSlot::default();
        }
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a < NUM_MOD_SLOTS && b < NUM_MOD_SLOTS && a != b {
            self.slots.swap(a, b);
        }
    }

    /// Evaluates all 32 slots for a voice's current `[0..1]` source values and accumulates
    /// per-destination normalized offsets into `out_deltas`. Zero heap allocations.
    #[inline]
    pub fn evaluate_destinations(
        &self,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
        out_deltas: &mut [f32; NUM_MOD_DESTS],
    ) {
        out_deltas.fill(0.0);
        for slot in &self.slots {
            if !slot.enabled || slot.bypassed {
                continue;
            }
            if let Some(src_idx) = slot.source.active_index() {
                let mut raw = source_values[src_idx].clamp(0.0, 1.0);
                if (slot.curve - 2.0).abs() < 0.1 {
                    raw = raw * raw * (3.0 - 2.0 * raw);
                } else if (slot.curve - 3.0).abs() < 0.1 {
                    raw = (raw * 4.0).floor() * 0.25;
                } else {
                    let curve = slot.curve.clamp(-1.0, 1.0);
                    if curve.abs() > 1e-4 {
                        let exp = (curve * 2.5).exp();
                        raw = raw.powf(exp);
                    }
                }
                let mut sig = if slot.bipolar {
                    raw * 2.0 - 1.0
                } else {
                    raw
                };
                if let Some(aux_idx) = slot.aux_source.active_index() {
                    sig *= source_values[aux_idx].clamp(0.0, 1.0);
                }
                let dest_idx = slot.destination.index();
                if dest_idx < NUM_MOD_DESTS {
                    out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
                }
            }
        }
    }
}

pub const FX_SCOPE_LEN: usize = 256;

/// Lock-free atomic ring buffer storing downsampled pre/post peak waveforms & activity for an FX slot.
pub struct FxScopeRing {
    pre_peaks: [AtomicU32; FX_SCOPE_LEN],
    post_peaks: [AtomicU32; FX_SCOPE_LEN],
    write_idx: AtomicU32,
    activity_bits: AtomicU32,
}

impl Default for FxScopeRing {
    fn default() -> Self {
        Self::new()
    }
}

impl FxScopeRing {
    pub fn new() -> Self {
        Self {
            pre_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            post_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            write_idx: AtomicU32::new(0),
            activity_bits: AtomicU32::new(0.0_f32.to_bits()),
        }
    }

    #[inline]
    pub fn push(&self, pre_peak: f32, post_peak: f32, activity: f32) {
        let idx = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        self.pre_peaks[idx].store(pre_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.post_peaks[idx].store(post_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.write_idx
            .store(((idx + 1) % FX_SCOPE_LEN) as u32, Ordering::Relaxed);
        self.activity_bits
            .store(activity.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn clear(&self) {
        for v in &self.pre_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        for v in &self.post_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        self.activity_bits
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    pub fn read_snapshot(
        &self,
        pre_out: &mut [f32; FX_SCOPE_LEN],
        post_out: &mut [f32; FX_SCOPE_LEN],
    ) -> f32 {
        let head = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        for i in 0..FX_SCOPE_LEN {
            let idx = (head + i) % FX_SCOPE_LEN;
            pre_out[i] = f32::from_bits(self.pre_peaks[idx].load(Ordering::Relaxed));
            post_out[i] = f32::from_bits(self.post_peaks[idx].load(Ordering::Relaxed));
        }
        f32::from_bits(self.activity_bits.load(Ordering::Relaxed))
    }
}

/// Lock-free atomic telemetry published by the audio thread for 60 Hz UI mod-ring animations.
pub struct ModUiTelemetry {
    pub dest_offsets: [AtomicU32; NUM_MOD_DESTS],
    pub source_levels: [AtomicU32; NUM_ACTIVE_SOURCES],
    pub modseq_phases: [AtomicU32; 4],
    pub env_levels: [AtomicU32; 4],
    /// Post-modulation `[cutoff_hz, resonance, drive, aux]` for `FILTER 1` and `FILTER 2`.
    pub filter_live: [[AtomicU32; 4]; 2],
    /// Lock-free FX scopes for `Bus A` (`0..8`), `Bus B` (`8..16`), and `Bus C` (`16..24`).
    pub fx_scopes: [FxScopeRing; 24],
    /// Lock-free Sub Bus clipper scope (`pre_driven`, `post_clipped`, `clip_activity`).
    pub sub_scope: FxScopeRing,
    /// Lock-free Master Output / Limiter scope (`pre_limit`, `post_out`, `limit_activity`).
    pub master_scope: FxScopeRing,
    pub master_peak_l: AtomicU32,
    pub master_peak_r: AtomicU32,
    pub master_rms_l: AtomicU32,
    pub master_rms_r: AtomicU32,
    pub master_clip_l: AtomicU32,
    pub master_clip_r: AtomicU32,
}

impl Default for ModUiTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModUiTelemetry {
    pub fn new() -> Self {
        Self {
            dest_offsets: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            source_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            modseq_phases: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            env_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            filter_live: std::array::from_fn(|_| {
                [
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                ]
            }),
            fx_scopes: std::array::from_fn(|_| FxScopeRing::new()),
            sub_scope: FxScopeRing::new(),
            master_scope: FxScopeRing::new(),
            master_peak_l: AtomicU32::new(0.0_f32.to_bits()),
            master_peak_r: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_l: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_r: AtomicU32::new(0.0_f32.to_bits()),
            master_clip_l: AtomicU32::new(0),
            master_clip_r: AtomicU32::new(0),
        }
    }

    #[inline]
    pub fn store_master_meters(
        &self,
        peak_l: f32,
        peak_r: f32,
        rms_l: f32,
        rms_r: f32,
        clip_l: bool,
        clip_r: bool,
    ) {
        self.master_peak_l.store(peak_l.to_bits(), Ordering::Relaxed);
        self.master_peak_r.store(peak_r.to_bits(), Ordering::Relaxed);
        self.master_rms_l.store(rms_l.to_bits(), Ordering::Relaxed);
        self.master_rms_r.store(rms_r.to_bits(), Ordering::Relaxed);
        if clip_l {
            self.master_clip_l.store(1, Ordering::Relaxed);
        }
        if clip_r {
            self.master_clip_r.store(1, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn master_meters(&self) -> (f32, f32, f32, f32, bool, bool) {
        (
            f32::from_bits(self.master_peak_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_peak_r.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_r.load(Ordering::Relaxed)),
            self.master_clip_l.load(Ordering::Relaxed) != 0,
            self.master_clip_r.load(Ordering::Relaxed) != 0,
        )
    }

    #[inline]
    pub fn reset_master_clip(&self) {
        self.master_clip_l.store(0, Ordering::Relaxed);
        self.master_clip_r.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn store_filter_live(
        &self,
        idx: usize,
        cutoff_hz: f32,
        resonance: f32,
        drive: f32,
        aux: f32,
    ) {
        if idx < 2 {
            self.filter_live[idx][0].store(cutoff_hz.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][1].store(resonance.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][2].store(drive.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][3].store(aux.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn filter_live(&self, idx: usize) -> [f32; 4] {
        if idx < 2 {
            [
                f32::from_bits(self.filter_live[idx][0].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][1].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][2].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][3].load(Ordering::Relaxed)),
            ]
        } else {
            [0.0; 4]
        }
    }

    #[inline]
    pub fn store_voice_snapshot(
        &self,
        dest_offsets: &[f32; NUM_MOD_DESTS],
        source_levels: &[f32; NUM_ACTIVE_SOURCES],
        modseq_phases: &[f32; 4],
        env_levels: &[f32; 4],
    ) {
        for (dst, &val) in self.dest_offsets.iter().zip(dest_offsets.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.source_levels.iter().zip(source_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.modseq_phases.iter().zip(modseq_phases.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.env_levels.iter().zip(env_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn fx_scope(&self, bus: usize, slot: usize) -> &FxScopeRing {
        let idx = bus.min(2) * 8 + slot.min(7);
        &self.fx_scopes[idx]
    }

    #[inline]
    pub fn dest_offset(&self, dest: ModDestination) -> f32 {
        f32::from_bits(self.dest_offsets[dest.index()].load(Ordering::Relaxed))
    }

    #[inline]
    pub fn source_level(&self, src: ModMatrixSource) -> f32 {
        if let Some(idx) = src.active_index() {
            f32::from_bits(self.source_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn modseq_phase(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.modseq_phases[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn env_level(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.env_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }
}
