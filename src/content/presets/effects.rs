//! Our effect banks, `Presets/<module folder>/<Name>.flmpst`: a flat list of presets per effect, our own settings and
//! names. Each is a set of the effect's params over its defaults; the registry's names resolve them (`#n` where a name
//! is shared: the effects' own `Mix` is also the common one).
//!
//! Laws, by the param tables (`params::effects`): the EQ and Graphic EQ gains are 24 v − 12 dB (0.5 flat); an EQ band's
//! frequency is 32·608.8^v Hz (0.2 → 115 Hz, 0.4 → 0.4 kHz, 0.6 → 1.5 kHz, 0.8 → 5.4 kHz); Compressor `Thresh` is
//! 40 v − 40 dB, `Ratio` 1 + 19 v² : 1 (1 is limiting), `Attack` 50 v² ms, `Release` 10 + 490 v² ms; the Filter's
//! frequency is 16·1260.7^v Hz and its menus truncate N v (`Kind` 0 LP, 0.6 BP, 1 HP; `Slope` 0.2 24 dB, 0.6 48 dB, 1 96 dB);
//! Distortion's `Kind` truncates 3 v (0 Hard, 0.4 Tube, 0.7 Soft, 1 Clip); Phaser's `LFO Rate` truncates 12 v (an
//! index i sits at (i + 0.5) / 12); Tape Delay's `Time` is 1000 v² ms.

use super::{Preset, p};

/// The effect banks: a module type and its presets.
pub(super) const BANKS: &[(u16, &[Preset])] = &[
    (3, EQUALIZER),
    (6, DISTORTION),
    (7, GRAPHIC_EQ),
    (8, COMPRESSOR),
    (9, FILTER),
    (11, CHORUS),
    (13, FLANGER),
    (14, PHASER),
    (15, STEREOIZER),
    (18, TAPE_DELAY),
];

const EQUALIZER: &[Preset] = &[
    p(
        "",
        "Vocal Presence",
        &[
            ("Gain 1", 0.333),
            ("Frequency 1", 0.15),
            ("Gain 3", 0.625),
            ("Gain 4", 0.583),
        ],
    ),
    p(
        "",
        "Bass Boost",
        &[
            ("Gain 1", 0.708),
            ("Frequency 1", 0.2),
            ("Gain 2", 0.583),
            ("Frequency 2", 0.3),
        ],
    ),
    p(
        "",
        "Air Lift",
        &[("Gain 4", 0.667), ("Frequency 4", 0.9), ("Gain 3", 0.54)],
    ),
    p(
        "",
        "Mud Cut",
        &[
            ("Gain 2", 0.292),
            ("Frequency 2", 0.35),
            ("Bandwidth 2", 0.4),
        ],
    ),
    p(
        "",
        "Telephone",
        &[
            ("Gain 1", 0.0),
            ("Frequency 1", 0.3),
            ("Kind 1", 0.0),
            ("Gain 3", 0.75),
            ("Frequency 3", 0.55),
            ("Gain 4", 0.0),
            ("Frequency 4", 0.75),
            ("Kind 4", 1.0),
        ],
    ),
    p(
        "",
        "Smile Curve",
        &[
            ("Gain 1", 0.667),
            ("Gain 2", 0.417),
            ("Gain 3", 0.417),
            ("Gain 4", 0.667),
        ],
    ),
    p(
        "",
        "Warm Low Shelf",
        &[("Kind 1", 0.0), ("Gain 1", 0.62), ("Frequency 1", 0.25)],
    ),
    p(
        "",
        "Bright Shelf",
        &[("Kind 4", 1.0), ("Gain 4", 0.66), ("Frequency 4", 0.7)],
    ),
];

const DISTORTION: &[Preset] = &[
    p(
        "",
        "Warm Tube",
        &[("Kind", 0.4), ("Drive", 0.35), ("Gain", 0.85)],
    ),
    p(
        "",
        "Crunch",
        &[("Kind", 0.0), ("Drive", 0.6), ("Gain", 0.8)],
    ),
    p(
        "",
        "Fuzz Box",
        &[
            ("Kind", 1.0),
            ("Drive", 0.85),
            ("Gain", 0.65),
            ("Bias", 0.56),
        ],
    ),
    p(
        "",
        "Soft Saturation",
        &[("Kind", 0.7), ("Drive", 0.3), ("Gain", 0.9)],
    ),
    p(
        "",
        "Lo-Fi Crush",
        &[("BitCrush", 0.55), ("Drive", 0.3), ("Kind", 0.7)],
    ),
    p(
        "",
        "Telephone Grit",
        &[
            ("Drive", 0.5),
            ("Kind", 0.0),
            ("BitCrush", 0.2),
            ("Mix", 0.6),
        ],
    ),
    p(
        "",
        "Parallel Drive",
        &[("Drive", 0.7), ("Kind", 0.4), ("Mix", 0.4)],
    ),
    p(
        "",
        "Wide Overdrive",
        &[
            ("Stereo", 0.8),
            ("Drive", 0.5),
            ("Kind", 0.4),
            ("Gain", 0.8),
        ],
    ),
];

const GRAPHIC_EQ: &[Preset] = &[
    p(
        "",
        "Smiley",
        &[
            ("80 Hz", 0.667),
            ("160 Hz", 0.583),
            ("375 Hz", 0.417),
            ("750 Hz", 0.375),
            ("1.5 kHz", 0.417),
            ("3 kHz", 0.542),
            ("6 kHz", 0.625),
            ("12 kHz", 0.667),
        ],
    ),
    p(
        "",
        "Bass Boost",
        &[("80 Hz", 0.75), ("160 Hz", 0.667), ("375 Hz", 0.583)],
    ),
    p(
        "",
        "Treble Boost",
        &[("3 kHz", 0.54), ("6 kHz", 0.667), ("12 kHz", 0.75)],
    ),
    p(
        "",
        "Vocal Clarity",
        &[
            ("160 Hz", 0.375),
            ("375 Hz", 0.417),
            ("3 kHz", 0.625),
            ("6 kHz", 0.583),
        ],
    ),
    p(
        "",
        "Mud Remover",
        &[("160 Hz", 0.417), ("375 Hz", 0.292), ("750 Hz", 0.417)],
    ),
    p(
        "",
        "Lo-Fi Radio",
        &[
            ("80 Hz", 0.0),
            ("160 Hz", 0.167),
            ("750 Hz", 0.667),
            ("1.5 kHz", 0.75),
            ("12 kHz", 0.083),
        ],
    ),
    p("", "Rumble Cut", &[("80 Hz", 0.0), ("160 Hz", 0.333)]),
    p(
        "",
        "Presence Dip",
        &[("1.5 kHz", 0.375), ("3 kHz", 0.333), ("6 kHz", 0.417)],
    ),
];

const COMPRESSOR: &[Preset] = &[
    p(
        "",
        "Gentle Glue",
        &[
            ("Thresh", 0.6),
            ("Ratio", 0.3),
            ("Attack", 0.6),
            ("Release", 0.55),
            ("Level", 0.55),
        ],
    ),
    p(
        "",
        "Vocal Leveler",
        &[
            ("Thresh", 0.5),
            ("Ratio", 0.4),
            ("Attack", 0.4),
            ("Release", 0.5),
            ("Level", 0.6),
        ],
    ),
    p(
        "",
        "Drum Smash",
        &[
            ("Thresh", 0.35),
            ("Ratio", 0.55),
            ("Attack", 0.25),
            ("Release", 0.3),
            ("Level", 0.65),
        ],
    ),
    p(
        "",
        "Bass Control",
        &[
            ("Thresh", 0.55),
            ("Ratio", 0.45),
            ("Attack", 0.55),
            ("Release", 0.6),
            ("Level", 0.55),
        ],
    ),
    p(
        "",
        "Pump",
        &[
            ("Thresh", 0.3),
            ("Ratio", 0.6),
            ("Attack", 0.05),
            ("Release", 0.85),
            ("Level", 0.6),
        ],
    ),
    p(
        "",
        "Transparent",
        &[
            ("Thresh", 0.7),
            ("Ratio", 0.2),
            ("Attack", 0.7),
            ("Release", 0.6),
        ],
    ),
    p(
        "",
        "Brickwall",
        &[
            ("Thresh", 0.75),
            ("Ratio", 1.0),
            ("Attack", 0.0),
            ("Release", 0.4),
            ("Level", 0.55),
        ],
    ),
    p(
        "",
        "Mix Bus Glue",
        &[
            ("Thresh", 0.65),
            ("Ratio", 0.22),
            ("Attack", 0.75),
            ("Release", 0.65),
            ("RMS", 1.0),
        ],
    ),
    p(
        "",
        "Parallel Squash",
        &[
            ("Thresh", 0.25),
            ("Ratio", 0.75),
            ("Attack", 0.2),
            ("Release", 0.4),
            ("Mix", 0.5),
        ],
    ),
];

const FILTER: &[Preset] = &[
    p(
        "",
        "Dark Lowpass",
        &[("Frequency", 0.5), ("Resonance", 0.1), ("Kind", 0.0)],
    ),
    p(
        "",
        "Telephone Band",
        &[
            ("Kind", 0.6),
            ("Frequency", 0.6),
            ("Resonance", 0.35),
            ("Slope", 0.2),
        ],
    ),
    p(
        "",
        "Rumble Cut",
        &[("Kind", 1.0), ("Frequency", 0.3), ("Resonance", 0.0)],
    ),
    p(
        "",
        "Resonant Sweep",
        &[("Frequency", 0.55), ("Resonance", 0.7), ("Kind", 0.0)],
    ),
    p(
        "",
        "Air Highpass",
        &[("Kind", 1.0), ("Frequency", 0.75), ("Resonance", 0.15)],
    ),
    p(
        "",
        "Muffled",
        &[("Frequency", 0.35), ("Resonance", 0.0), ("Slope", 1.0)],
    ),
    p(
        "",
        "Wah Peak",
        &[
            ("Kind", 0.6),
            ("Frequency", 0.5),
            ("Resonance", 0.8),
            ("Slope", 0.2),
        ],
    ),
    p(
        "",
        "Sub Only",
        &[("Frequency", 0.22), ("Resonance", 0.2), ("Slope", 1.0)],
    ),
];

const CHORUS: &[Preset] = &[
    p(
        "",
        "Classic Ensemble",
        &[("Rate", 0.3), ("Depth", 0.5), ("Stereo", 1.0), ("#4", 0.5)],
    ),
    p(
        "",
        "Slow Shimmer",
        &[("Rate", 0.1), ("Depth", 0.7), ("#4", 0.45)],
    ),
    p(
        "",
        "Fast Vibrato",
        &[("Rate", 0.85), ("Depth", 0.3), ("#4", 1.0)],
    ),
    p(
        "",
        "Wide Pad",
        &[("Rate", 0.25), ("Depth", 0.6), ("Stereo", 1.0), ("#4", 0.6)],
    ),
    p(
        "",
        "Subtle Double",
        &[
            ("Rate", 0.35),
            ("Depth", 0.2),
            ("Stereo", 0.6),
            ("#4", 0.35),
        ],
    ),
    p(
        "",
        "Lush Deep",
        &[("Rate", 0.2), ("Depth", 0.9), ("#4", 0.7)],
    ),
    p(
        "",
        "Mono Chorus",
        &[("Rate", 0.4), ("Depth", 0.5), ("Stereo", 0.0), ("#4", 0.5)],
    ),
];

const FLANGER: &[Preset] = &[
    p(
        "",
        "Jet Sweep",
        &[
            ("LFO Speed", 0.4),
            ("LFO Amount", 0.9),
            ("Delay", 0.3),
            ("Feedback", 0.8),
            ("Stereo", 0.3),
        ],
    ),
    p(
        "",
        "Slow Tape",
        &[
            ("LFO Speed", 0.25),
            ("LFO Amount", 0.8),
            ("Delay", 0.35),
            ("Feedback", 0.6),
        ],
    ),
    p(
        "",
        "Metallic Resonance",
        &[
            ("LFO Speed", 0.3),
            ("LFO Amount", 0.5),
            ("Delay", 0.1),
            ("Feedback", 0.9),
        ],
    ),
    p(
        "",
        "Subtle Swirl",
        &[("LFO Amount", 0.4), ("Feedback", 0.4), ("#10", 0.5)],
    ),
    p(
        "",
        "Wide Stereo Flange",
        &[("Stereo", 0.8), ("LFO Speed", 0.45), ("Feedback", 0.65)],
    ),
    p(
        "",
        "Through-Zero Feel",
        &[("Inv. FDBK", 0.0), ("Delay", 0.2), ("Feedback", 0.7)],
    ),
    p(
        "",
        "Fast Wobble",
        &[("LFO Speed", 0.75), ("LFO Amount", 0.5), ("Delay", 0.2)],
    ),
];

const PHASER: &[Preset] = &[
    p(
        "",
        "Slow Sweep",
        &[("LFO Rate", 0.625), ("LFO Amount", 0.7), ("Feedback", 0.5)],
    ),
    p(
        "",
        "Classic Phase",
        &[
            ("LFO Rate", 0.375),
            ("LFO Amount", 0.5),
            ("Feedback", 0.3),
            ("#5", 0.6),
        ],
    ),
    p(
        "",
        "Deep Jet",
        &[("LFO Rate", 0.458), ("LFO Amount", 0.9), ("Feedback", 0.7)],
    ),
    p(
        "",
        "Subtle Movement",
        &[
            ("LFO Rate", 0.708),
            ("LFO Amount", 0.3),
            ("Feedback", 0.15),
            ("#5", 0.5),
        ],
    ),
    p(
        "",
        "Fast Phaser",
        &[("LFO Rate", 0.125), ("LFO Amount", 0.6), ("Feedback", 0.4)],
    ),
    p(
        "",
        "Wide Phase",
        &[("Stereo", 1.0), ("LFO Amount", 0.6), ("Feedback", 0.35)],
    ),
];

const STEREOIZER: &[Preset] = &[
    p(
        "",
        "Wide Open",
        &[("Strength", 1.0), ("Timbre", 0.6), ("Focus", 0.5)],
    ),
    p("", "Subtle Width", &[("Strength", 0.4)]),
    p("", "Haas Stereo", &[("L/R Delay", 0.8), ("Strength", 0.6)]),
    p(
        "",
        "Mono-Safe Wide",
        &[("Strength", 0.5), ("Timbre", 0.4), ("Focus", 0.5)],
    ),
    p("", "Dark Width", &[("Timbre", 0.25), ("Strength", 0.8)]),
    p("", "Bright Air", &[("Timbre", 0.85), ("Strength", 0.7)]),
];

const TAPE_DELAY: &[Preset] = &[
    p(
        "",
        "Slapback",
        &[("Time", 0.28), ("Fdbk", 0.15), ("#4", 0.3)],
    ),
    p(
        "",
        "Eighth Echo",
        &[("Time", 0.5), ("Fdbk", 0.4), ("#4", 0.35)],
    ),
    p(
        "",
        "Dotted Dub",
        &[("Time", 0.61), ("Fdbk", 0.6), ("#4", 0.4), ("Async.", 0.55)],
    ),
    p("", "Quarter Tape", &[("Time", 0.7), ("Fdbk", 0.45)]),
    p(
        "",
        "Long Space",
        &[("Time", 0.87), ("Fdbk", 0.7), ("#4", 0.45)],
    ),
    p(
        "",
        "Ping Wide",
        &[("Async.", 0.75), ("Time", 0.5), ("Fdbk", 0.5)],
    ),
    p(
        "",
        "Runaway",
        &[("Fdbk", 0.92), ("Time", 0.55), ("#4", 0.5)],
    ),
    p(
        "",
        "Subtle Doubler",
        &[("Time", 0.15), ("Fdbk", 0.05), ("#4", 0.3)],
    ),
];
