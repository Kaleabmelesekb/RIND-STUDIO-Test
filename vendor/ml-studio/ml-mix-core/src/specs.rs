use ml_small_fx::{Card, Control, Mark, Setting, Spec};
pub static SPLIT: Spec = Spec {
    name: "ML Split",
    word: "SPLIT",
    about: "ML Studio Split. See included source and third-party notices.",
    settings: &[
        Setting {
            id: "low",
            name: "Low / Mid",
            range: (20, 5000),
            start: 250,
            magnet: None,
        },
        Setting {
            id: "high",
            name: "Mid / High",
            range: (100, 20000),
            start: 4000,
            magnet: None,
        },
        Setting {
            id: "low_gain",
            name: "Low gain",
            range: (-240, 120),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "mid_gain",
            name: "Mid gain",
            range: (-240, 120),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "high_gain",
            name: "High gain",
            range: (-240, 120),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "output",
            name: "Output",
            range: (-240, 120),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "band",
            name: "Band",
            range: (0, 3),
            start: 0,
            magnet: None,
        },
    ],
    studio_settings: 7,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 330),
    cards: &[
        Card {
            x0: 8.,
            x1: 124.,
            y0: 28.,
            y1: 153.,
            heading: "LOW / MID",
        },
        Card {
            x0: 132.,
            x1: 248.,
            y0: 28.,
            y1: 153.,
            heading: "MID / HIGH",
        },
        Card {
            x0: 256.,
            x1: 372.,
            y0: 28.,
            y1: 153.,
            heading: "LOW GAIN",
        },
        Card {
            x0: 380.,
            x1: 496.,
            y0: 28.,
            y1: 153.,
            heading: "MID GAIN",
        },
        Card {
            x0: 504.,
            x1: 620.,
            y0: 28.,
            y1: 153.,
            heading: "HIGH GAIN",
        },
        Card {
            x0: 628.,
            x1: 744.,
            y0: 28.,
            y1: 153.,
            heading: "OUTPUT",
        },
        Card {
            x0: 8.,
            x1: 150.,
            y0: 182.,
            y1: 280.,
            heading: "OUTPUT BAND",
        },
    ],
    controls: &[
        Control::Knob {
            id: "low",
            cx: 66.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high",
            cx: 190.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "low_gain",
            cx: 314.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "mid_gain",
            cx: 438.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "high_gain",
            cx: 562.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "output",
            cx: 686.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "band",
            x: 20.,
            top: 191.,
            names: &["All", "Low", "Mid", "High"],
        },
    ],
    display: None,
    mark: Mark {
        x: 744.,
        y: 312.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: split_text,
    parse: split_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn split_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{v} Hz"),
        1 => format!("{v} Hz"),
        2 => format!("{:+.1} dB", v as f64 / 10.),
        3 => format!("{:+.1} dB", v as f64 / 10.),
        4 => format!("{:+.1} dB", v as f64 / 10.),
        5 => format!("{:+.1} dB", v as f64 / 10.),
        6 => ["All", "Low", "Mid", "High"][v.clamp(0, 3) as usize].into(),
        _ => String::new(),
    }
}
fn split_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 6 {
        return ["all", "low", "mid", "high"]
            .iter()
            .position(|x| *x == s.to_lowercase())
            .map(|x| x as i32);
    }
    let v = s
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    if !v.is_finite() {
        return None;
    }
    let scale = if k < 2 { 1. } else { 10. };
    let p = SPLIT.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}
pub static SHAPER: Spec = Spec {
    name: "ML Waveshaper",
    word: "WAVESHAPER",
    about: "ML Studio Waveshaper. See included source and third-party notices.",
    settings: &[
        Setting {
            id: "drive",
            name: "Drive",
            range: (0, 360),
            start: 60,
            magnet: None,
        },
        Setting {
            id: "bias",
            name: "Bias",
            range: (-1000, 1000),
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
            range: (-360, 120),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "dc",
            name: "Remove DC",
            range: (0, 1),
            start: 1,
            magnet: None,
        },
        Setting {
            id: "p0",
            name: "Curve 1",
            range: (-1000, 1000),
            start: -1000,
            magnet: None,
        },
        Setting {
            id: "p1",
            name: "Curve 2",
            range: (-1000, 1000),
            start: -750,
            magnet: None,
        },
        Setting {
            id: "p2",
            name: "Curve 3",
            range: (-1000, 1000),
            start: -500,
            magnet: None,
        },
        Setting {
            id: "p3",
            name: "Curve 4",
            range: (-1000, 1000),
            start: -250,
            magnet: None,
        },
        Setting {
            id: "p4",
            name: "Curve 5",
            range: (-1000, 1000),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "p5",
            name: "Curve 6",
            range: (-1000, 1000),
            start: 250,
            magnet: None,
        },
        Setting {
            id: "p6",
            name: "Curve 7",
            range: (-1000, 1000),
            start: 500,
            magnet: None,
        },
        Setting {
            id: "p7",
            name: "Curve 8",
            range: (-1000, 1000),
            start: 750,
            magnet: None,
        },
        Setting {
            id: "p8",
            name: "Curve 9",
            range: (-1000, 1000),
            start: 1000,
            magnet: None,
        },
    ],
    studio_settings: 14,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (660, 480),
    cards: &[
        Card {
            x0: 8.,
            x1: 128.,
            y0: 28.,
            y1: 153.,
            heading: "DRIVE",
        },
        Card {
            x0: 136.,
            x1: 256.,
            y0: 28.,
            y1: 153.,
            heading: "BIAS",
        },
        Card {
            x0: 264.,
            x1: 384.,
            y0: 28.,
            y1: 153.,
            heading: "MIX",
        },
        Card {
            x0: 392.,
            x1: 512.,
            y0: 28.,
            y1: 153.,
            heading: "OUTPUT",
        },
        Card {
            x0: 520.,
            x1: 640.,
            y0: 28.,
            y1: 153.,
            heading: "REMOVE DC",
        },
    ],
    controls: &[
        Control::Knob {
            id: "drive",
            cx: 68.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "bias",
            cx: 196.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Knob {
            id: "mix",
            cx: 324.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "output",
            cx: 452.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "dc",
            x: 532.,
            top: 66.,
            names: &["Off", "On"],
        },
    ],
    display: None,
    mark: Mark {
        x: 644.,
        y: 462.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: shaper_text,
    parse: shaper_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn shaper_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:+.1} dB", v as f64 / 10.),
        1 => format!("{:.1}%", v as f64 / 10.),
        2 => format!("{:.1}%", v as f64 / 10.),
        3 => format!("{:+.1} dB", v as f64 / 10.),
        4 => {
            if v == 0 {
                "Off".into()
            } else {
                "On".into()
            }
        }
        5 => format!("{:.1}%", v as f64 / 10.),
        6 => format!("{:.1}%", v as f64 / 10.),
        7 => format!("{:.1}%", v as f64 / 10.),
        8 => format!("{:.1}%", v as f64 / 10.),
        9 => format!("{:.1}%", v as f64 / 10.),
        10 => format!("{:.1}%", v as f64 / 10.),
        11 => format!("{:.1}%", v as f64 / 10.),
        12 => format!("{:.1}%", v as f64 / 10.),
        13 => format!("{:.1}%", v as f64 / 10.),
        _ => String::new(),
    }
}
fn shaper_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 4 {
        return match s.to_lowercase().as_str() {
            "off" | "0" => Some(0),
            "on" | "1" => Some(1),
            _ => None,
        };
    }
    let v = s
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    if !v.is_finite() {
        return None;
    }
    let scale = 10.;
    let p = SHAPER.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}
pub static REFERENCE: Spec = Spec {
    name: "ML Reference",
    word: "REFERENCE",
    about: "ML Studio Reference. See included source and third-party notices.",
    settings: &[
        Setting {
            id: "start",
            name: "Start",
            range: (0, 12000),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "length",
            name: "Loop",
            range: (10, 12000),
            start: 300,
            magnet: None,
        },
        Setting {
            id: "trim",
            name: "Ref trim",
            range: (-180, 180),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "match",
            name: "Level match",
            range: (0, 1),
            start: 1,
            magnet: None,
        },
    ],
    studio_settings: 4,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (660, 420),
    cards: &[
        Card {
            x0: 8.,
            x1: 160.,
            y0: 28.,
            y1: 153.,
            heading: "START",
        },
        Card {
            x0: 168.,
            x1: 320.,
            y0: 28.,
            y1: 153.,
            heading: "LOOP",
        },
        Card {
            x0: 328.,
            x1: 480.,
            y0: 28.,
            y1: 153.,
            heading: "REF TRIM",
        },
        Card {
            x0: 488.,
            x1: 640.,
            y0: 28.,
            y1: 153.,
            heading: "LEVEL MATCH",
        },
    ],
    controls: &[
        Control::Knob {
            id: "start",
            cx: 84.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "length",
            cx: 244.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "trim",
            cx: 404.0,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "match",
            x: 500.,
            top: 66.,
            names: &["Off", "On"],
        },
    ],
    display: None,
    mark: Mark {
        x: 644.,
        y: 402.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: reference_text,
    parse: reference_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn reference_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:.1} s", v as f64 / 10.),
        1 => format!("{:.1} s", v as f64 / 10.),
        2 => format!("{:+.1} dB", v as f64 / 10.),
        3 => {
            if v == 0 {
                "Off".into()
            } else {
                "On".into()
            }
        }
        _ => String::new(),
    }
}
fn reference_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 3 {
        return match s.to_lowercase().as_str() {
            "off" | "0" => Some(0),
            "on" | "1" => Some(1),
            _ => None,
        };
    }
    let v = s
        .split_whitespace()
        .next()?
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    if !v.is_finite() {
        return None;
    }
    let scale = 10.;
    let p = REFERENCE.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}
