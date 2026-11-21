use ml_small_fx::{Card, Control, Mark, Setting, Spec};
fn number(s: &str) -> Option<f64> {
    let v = s
        .trim()
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    v.is_finite().then_some(v)
}
fn deess_text(k: usize, v: i32) -> String {
    match k {
        0 | 1 => format!("{:.1}%", v as f64 / 10.),
        2 => format!("{:+.1} dB", v as f64 / 10.),
        _ => if v == 0 { "Normal" } else { "Removed" }.into(),
    }
}
fn deess_parse(k: usize, s: &str) -> Option<i32> {
    if k == 3 {
        return match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(0),
            "removed" => Some(1),
            _ => None,
        };
    }
    let v = (number(s)? * 10.).round() as i32;
    Some(if k < 2 {
        v.clamp(0, 1000)
    } else {
        v.clamp(-240, 120)
    })
}
fn pitch_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{v:+} st"),
        1 => format!("{v:+} cents"),
        2 => if v == 0 { "Follow pitch" } else { "Preserve" }.into(),
        3 => format!("{:.1}%", v as f64 / 10.),
        _ => format!("{:+.1} dB", v as f64 / 10.),
    }
}
fn pitch_parse(k: usize, s: &str) -> Option<i32> {
    if k == 2 {
        return match s.trim().to_ascii_lowercase().as_str() {
            "follow pitch" => Some(0),
            "preserve" => Some(1),
            _ => None,
        };
    }
    let n = number(s)?;
    let v = if k >= 3 { (n * 10.).round() } else { n.round() } as i32;
    Some(match k {
        0 => v.clamp(-24, 24),
        1 => v.clamp(-100, 100),
        3 => v.clamp(0, 1000),
        _ => v.clamp(-240, 120),
    })
}
pub static DEESSER: Spec = Spec {
    name: "ML De-Esser",
    word: "DE-ESSER",
    about: "ML Studio de-esser. Airwindows DeBess DSP (Chris Johnson, MIT).",
    settings: &[
        Setting {
            id: "amount",
            name: "Amount",
            range: (0, 1000),
            start: 500,
            magnet: None,
        },
        Setting {
            id: "mix",
            name: "Mix",
            range: (0, 1000),
            start: 1000,
            magnet: None,
        },
        Setting {
            id: "output",
            name: "Output",
            range: (-240, 120),
            start: 0,
            magnet: Some(0),
        },
        Setting {
            id: "listen",
            name: "Listen",
            range: (0, 1),
            start: 0,
            magnet: None,
        },
    ],
    studio_settings: 4,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (600, 235),
    cards: &[
        Card {
            x0: 8.,
            x1: 108.,
            y0: 28.,
            y1: 155.,
            heading: "AMOUNT",
        },
        Card {
            x0: 116.,
            x1: 216.,
            y0: 28.,
            y1: 155.,
            heading: "MIX",
        },
        Card {
            x0: 224.,
            x1: 324.,
            y0: 28.,
            y1: 155.,
            heading: "OUTPUT",
        },
        Card {
            x0: 332.,
            x1: 592.,
            y0: 28.,
            y1: 155.,
            heading: "MONITOR",
        },
    ],
    controls: &[
        Control::Knob {
            id: "amount",
            cx: 58.,
            cy: 80.,
            r: 25.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mix",
            cx: 166.,
            cy: 80.,
            r: 25.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "output",
            cx: 274.,
            cy: 80.,
            r: 25.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "listen",
            x: 370.,
            top: 108.,
            names: &["Normal", "Removed"],
        },
    ],
    display: None,
    mark: Mark {
        x: 584.,
        y: 215.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: deess_text,
    parse: deess_parse,
    sound: |_, _| unreachable!("native processor"),
    rings: true,
};
pub static PITCH: Spec = Spec {
    name: "ML Pitch Shift",
    word: "PITCH SHIFT",
    about: "ML Studio pitch shift. Signalsmith Stretch / Geraint Luff and Colin Marc (MIT).",
    settings: &[
        Setting {
            id: "semitones",
            name: "Semitones",
            range: (-24, 24),
            start: 0,
            magnet: Some(0),
        },
        Setting {
            id: "cents",
            name: "Fine tune",
            range: (-100, 100),
            start: 0,
            magnet: Some(0),
        },
        Setting {
            id: "formants",
            name: "Formants",
            range: (0, 1),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "mix",
            name: "Mix",
            range: (0, 1000),
            start: 1000,
            magnet: None,
        },
        Setting {
            id: "output",
            name: "Output",
            range: (-240, 120),
            start: 0,
            magnet: Some(0),
        },
    ],
    studio_settings: 5,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (660, 235),
    cards: &[
        Card {
            x0: 8.,
            x1: 100.,
            y0: 28.,
            y1: 167.,
            heading: "SEMITONES",
        },
        Card {
            x0: 106.,
            x1: 198.,
            y0: 28.,
            y1: 167.,
            heading: "FINE TUNE",
        },
        Card {
            x0: 204.,
            x1: 296.,
            y0: 28.,
            y1: 167.,
            heading: "MIX",
        },
        Card {
            x0: 302.,
            x1: 394.,
            y0: 28.,
            y1: 167.,
            heading: "OUTPUT",
        },
        Card {
            x0: 402.,
            x1: 652.,
            y0: 28.,
            y1: 167.,
            heading: "FORMANTS",
        },
    ],
    controls: &[
        Control::Knob {
            id: "semitones",
            cx: 54.,
            cy: 82.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "cents",
            cx: 152.,
            cy: 82.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "mix",
            cx: 250.,
            cy: 82.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "output",
            cx: 348.,
            cy: 82.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "formants",
            x: 430.,
            top: 60.,
            names: &["Follow pitch", "Preserve"],
        },
    ],
    display: None,
    mark: Mark {
        x: 652.,
        y: 215.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: pitch_text,
    parse: pitch_parse,
    sound: |_, _| unreachable!("native processor"),
    rings: true,
};
pub static METER: Spec = Spec {
    name: "ML Meter",
    word: "METER",
    about: "ML Studio loudness, peak and stereo meter. EBU R128 library (MIT).",
    settings: &[],
    studio_settings: 0,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (600, 350),
    cards: &[
        Card {
            x0: 8.,
            x1: 196.,
            y0: 28.,
            y1: 207.,
            heading: "LEFT",
        },
        Card {
            x0: 204.,
            x1: 392.,
            y0: 28.,
            y1: 207.,
            heading: "RIGHT",
        },
        Card {
            x0: 400.,
            x1: 592.,
            y0: 28.,
            y1: 207.,
            heading: "LOUDNESS",
        },
        Card {
            x0: 8.,
            x1: 592.,
            y0: 218.,
            y1: 275.,
            heading: "",
        },
    ],
    controls: &[],
    display: None,
    mark: Mark {
        x: 584.,
        y: 332.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: |_, _| String::new(),
    parse: |_, _| None,
    sound: |_, _| unreachable!("native processor"),
    rings: true,
};
