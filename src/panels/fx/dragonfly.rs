//! The Dragonfly reverbs (`honeydew_audio::dragonfly_fx`), UIHeight 7 U: Honeydew's own panels, a tab strip over pages
//! of knobs, with each plugin's own knobs, names and order. The plugins are Michael Willis and Rob van den Berg's Dragonfly
//! Reverb (GPL-3.0-or-later); their own OpenGL window needs a desktop GL context, so these panels stand in for it.

use honeydew_audio::dragonfly_fx::{early as e, hall as h, plate as p, room as r};

use crate::panels::build::*;

/// Pages `MIX` (0), `SPACE` (1) and `TONE` (2).
pub(in crate::panels) fn hall() -> Panel {
    Panel::new(vstack([
        tabs(1, "MIX · SPACE · TONE").h(Size::U(1.25)),
        hstack([
            knob(2, "Dry Level").p(h::DRY_LEVEL),
            knob(3, "Early Level").p(h::EARLY_LEVEL),
            knob(4, "Late Level").p(h::LATE_LEVEL),
            knob(5, "Early Send").p(h::EARLY_SEND),
            knob(6, "Width").p(h::WIDTH),
            knob(7, "Predelay").p(h::PREDELAY),
        ])
        .show(Show::Tab(0)),
        hstack([
            knob(8, "Size").p(h::SIZE),
            knob(9, "Decay").p(h::DECAY),
            knob(10, "Diffuse").p(h::DIFFUSE),
            knob(11, "Spin").p(h::SPIN),
            knob(12, "Wander").p(h::WANDER),
            knob(13, "Modulation").p(h::MODULATION),
        ])
        .show(Show::Tab(1)),
        hstack([
            knob(14, "Low Cut").p(h::LOW_CUT),
            knob(15, "Low Cross").p(h::LOW_CROSS),
            knob(16, "Low Mult").p(h::LOW_MULT),
            knob(17, "High Cut").p(h::HIGH_CUT),
            knob(18, "High Cross").p(h::HIGH_CROSS),
            knob(19, "High Mult").p(h::HIGH_MULT),
        ])
        .show(Show::Tab(2)),
    ]))
}

/// Pages `MIX` (0), `SPACE` (1) and `TONE` (2).
pub(in crate::panels) fn room() -> Panel {
    Panel::new(vstack([
        tabs(1, "MIX · SPACE · TONE").h(Size::U(1.25)),
        hstack([
            knob(2, "Dry Level").p(r::DRY_LEVEL),
            knob(3, "Early Level").p(r::EARLY_LEVEL),
            knob(4, "Early Send").p(r::EARLY_SEND),
            knob(5, "Late Level").p(r::LATE_LEVEL),
            knob(6, "Width").p(r::WIDTH),
            knob(7, "Predelay").p(r::PREDELAY),
        ])
        .show(Show::Tab(0)),
        hstack([
            knob(8, "Size").p(r::SIZE),
            knob(9, "Decay").p(r::DECAY),
            knob(10, "Diffuse").p(r::DIFFUSE),
            knob(11, "Spin").p(r::SPIN),
            knob(12, "Wander").p(r::WANDER),
        ])
        .show(Show::Tab(1)),
        hstack([
            knob(13, "High Cut").p(r::HIGH_CUT),
            knob(14, "Early Damp").p(r::EARLY_DAMP),
            knob(15, "Late Damp").p(r::LATE_DAMP),
            knob(16, "Low Cut").p(r::LOW_CUT),
            knob(17, "Low Boost").p(r::LOW_BOOST),
            knob(18, "Boost Freq").p(r::BOOST_FREQ),
        ])
        .show(Show::Tab(2)),
    ]))
}

/// Pages `MIX` (0) and `TONE` (1); the Algorithm list is on the first.
pub(in crate::panels) fn plate() -> Panel {
    Panel::new(vstack([
        tabs(1, "MIX · TONE").h(Size::U(1.25)),
        hstack([
            vstack([list(2, "Simple · Nested · Tank", Bar::Left).p(p::ALGORITHM)]).w(Size::U(3.5)),
            knob(3, "Dry Level").p(p::DRY_LEVEL),
            knob(4, "Wet Level").p(p::WET_LEVEL),
            knob(5, "Width").p(p::WIDTH),
            knob(6, "Predelay").p(p::PREDELAY),
            knob(7, "Decay").p(p::DECAY),
        ])
        .show(Show::Tab(0)),
        hstack([
            knob(8, "Low Cut").p(p::LOW_CUT),
            knob(9, "High Cut").p(p::HIGH_CUT),
            knob(10, "Dampen").p(p::DAMPEN),
        ])
        .show(Show::Tab(1)),
    ]))
}

/// Pages `MIX` (0) and `SHAPE` (1); the Program spinner is on the first.
pub(in crate::panels) fn early() -> Panel {
    Panel::new(vstack([
        tabs(1, "MIX · SHAPE").h(Size::U(1.25)),
        hstack([
            vstack([spinner(2, "Program")
                .p(e::PROGRAM)
                .items_per_column(8)
                .h(Size::U(1.25))])
            .w(Size::U(4.0)),
            knob(3, "Dry Level").p(e::DRY_LEVEL),
            knob(4, "Wet Level").p(e::WET_LEVEL),
        ])
        .show(Show::Tab(0)),
        hstack([
            knob(5, "Size").p(e::SIZE),
            knob(6, "Width").p(e::WIDTH),
            knob(7, "Low Cut").p(e::LOW_CUT),
            knob(8, "High Cut").p(e::HIGH_CUT),
        ])
        .show(Show::Tab(1)),
    ]))
}
