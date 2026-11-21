use ml_small_fx::{Card, Control, Mark, Setting, Spec};
pub static FREQUENCY: Spec = Spec {
    name: "ML Frequency Shift",
    word: "FREQUENCY SHIFT",
    about: "ML Studio Frequency Shift. See included source and notices.",
    settings: &[
        Setting {
            id: "shift",
            name: "Shift",
            range: (-20000, 20000),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "stereo",
            name: "Stereo",
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
            magnet: None,
        },
    ],
    studio_settings: 4,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (680, 280),
    cards: &[
        Card {
            x0: 8.,
            x1: 166.,
            y0: 28.,
            y1: 153.,
            heading: "SHIFT",
        },
        Card {
            x0: 174.,
            x1: 332.,
            y0: 28.,
            y1: 153.,
            heading: "STEREO",
        },
        Card {
            x0: 340.,
            x1: 498.,
            y0: 28.,
            y1: 153.,
            heading: "MIX",
        },
        Card {
            x0: 506.,
            x1: 664.,
            y0: 28.,
            y1: 153.,
            heading: "OUTPUT",
        },
    ],
    controls: &[
        Control::Knob {
            id: "shift",
            cx: 87.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "stereo",
            x: 186.,
            top: 66.,
            names: &["Same", "Opposite"],
        },
        Control::Knob {
            id: "mix",
            cx: 419.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "output",
            cx: 585.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
    ],
    display: None,
    mark: Mark {
        x: 664.,
        y: 262.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: frequency_text,
    parse: frequency_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn frequency_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:.1} Hz", v as f64 / 10.),
        1 => ["Same", "Opposite"][v.clamp(0, 1) as usize].into(),
        2 => format!("{:.1}%", v as f64 / 10.),
        3 => format!("{:+.1} dB", v as f64 / 10.),
        _ => String::new(),
    }
}
fn frequency_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 1 {
        return ["same", "opposite"]
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
    let scale = if [0, 2, 3].contains(&k) { 10. } else { 1. };
    let p = FREQUENCY.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}
pub static TUNER: Spec = Spec {
    name: "ML Tuner",
    word: "TUNER",
    about: "ML Studio Tuner. See included source and notices.",
    settings: &[
        Setting {
            id: "a4",
            name: "A4",
            range: (4000, 4800),
            start: 4400,
            magnet: None,
        },
        Setting {
            id: "gate",
            name: "Gate",
            range: (-90, -20),
            start: -60,
            magnet: None,
        },
        Setting {
            id: "input",
            name: "Input",
            range: (0, 2),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "mute",
            name: "Mute",
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
    size: (660, 412),
    cards: &[
        Card {
            x0: 8.,
            x1: 161.,
            y0: 28.,
            y1: 153.,
            heading: "A4",
        },
        Card {
            x0: 169.,
            x1: 322.,
            y0: 28.,
            y1: 153.,
            heading: "GATE",
        },
        Card {
            x0: 330.,
            x1: 483.,
            y0: 28.,
            y1: 153.,
            heading: "INPUT",
        },
        Card {
            x0: 491.,
            x1: 644.,
            y0: 28.,
            y1: 153.,
            heading: "MUTE",
        },
    ],
    controls: &[
        Control::Knob {
            id: "a4",
            cx: 84.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "gate",
            cx: 245.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Choice {
            id: "input",
            x: 342.,
            top: 66.,
            names: &["Left", "Right", "Mid"],
        },
        Control::Choice {
            id: "mute",
            x: 503.,
            top: 66.,
            names: &["Off", "On"],
        },
    ],
    display: None,
    mark: Mark {
        x: 644.,
        y: 394.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: tuner_text,
    parse: tuner_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn tuner_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:.1} Hz", v as f64 / 10.),
        1 => format!("{v} dB"),
        2 => ["Left", "Right", "Mid"][v.clamp(0, 2) as usize].into(),
        3 => ["Off", "On"][v.clamp(0, 1) as usize].into(),
        _ => String::new(),
    }
}
fn tuner_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 2 {
        return ["left", "right", "mid"]
            .iter()
            .position(|x| *x == s.to_lowercase())
            .map(|x| x as i32);
    }
    if k == 3 {
        return ["off", "on"]
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
    let scale = if [0].contains(&k) { 10. } else { 1. };
    let p = TUNER.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}
pub static MULTIDELAY: Spec = Spec {
    name: "ML Multiband Delay",
    word: "MULTIBAND DELAY",
    about: "ML Studio Multiband Delay. See included source and notices.",
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
            id: "mix",
            name: "Mix",
            range: (0, 1000),
            start: 250,
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
            id: "sync",
            name: "Sync",
            range: (0, 1),
            start: 1,
            magnet: None,
        },
        Setting {
            id: "low_time",
            name: "Low time",
            range: (1, 4000),
            start: 250,
            magnet: None,
        },
        Setting {
            id: "low_beat",
            name: "Low beats",
            range: (0, 7),
            start: 4,
            magnet: None,
        },
        Setting {
            id: "low_feedback",
            name: "Low feedback",
            range: (0, 900),
            start: 300,
            magnet: None,
        },
        Setting {
            id: "low_level",
            name: "Low level",
            range: (-600, 60),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "mid_time",
            name: "Mid time",
            range: (1, 4000),
            start: 375,
            magnet: None,
        },
        Setting {
            id: "mid_beat",
            name: "Mid beats",
            range: (0, 7),
            start: 3,
            magnet: None,
        },
        Setting {
            id: "mid_feedback",
            name: "Mid feedback",
            range: (0, 900),
            start: 300,
            magnet: None,
        },
        Setting {
            id: "mid_level",
            name: "Mid level",
            range: (-600, 60),
            start: 0,
            magnet: None,
        },
        Setting {
            id: "high_time",
            name: "High time",
            range: (1, 4000),
            start: 500,
            magnet: None,
        },
        Setting {
            id: "high_beat",
            name: "High beats",
            range: (0, 7),
            start: 2,
            magnet: None,
        },
        Setting {
            id: "high_feedback",
            name: "High feedback",
            range: (0, 900),
            start: 300,
            magnet: None,
        },
        Setting {
            id: "high_level",
            name: "High level",
            range: (-600, 60),
            start: 0,
            magnet: None,
        },
    ],
    studio_settings: 17,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    size: (760, 694),
    cards: &[
        Card {
            x0: 8.,
            x1: 150.,
            y0: 28.,
            y1: 153.,
            heading: "LOW / MID",
        },
        Card {
            x0: 158.,
            x1: 300.,
            y0: 28.,
            y1: 153.,
            heading: "MID / HIGH",
        },
        Card {
            x0: 308.,
            x1: 450.,
            y0: 28.,
            y1: 153.,
            heading: "MIX",
        },
        Card {
            x0: 458.,
            x1: 600.,
            y0: 28.,
            y1: 153.,
            heading: "OUTPUT",
        },
        Card {
            x0: 608.,
            x1: 750.,
            y0: 28.,
            y1: 153.,
            heading: "SYNC",
        },
        Card {
            x0: 142.,
            x1: 284.,
            y0: 185.,
            y1: 310.,
            heading: "TIME",
        },
        Card {
            x0: 292.,
            x1: 434.,
            y0: 185.,
            y1: 310.,
            heading: "BEATS",
        },
        Card {
            x0: 442.,
            x1: 584.,
            y0: 185.,
            y1: 310.,
            heading: "FEEDBACK",
        },
        Card {
            x0: 592.,
            x1: 734.,
            y0: 185.,
            y1: 310.,
            heading: "LEVEL",
        },
        Card {
            x0: 142.,
            x1: 284.,
            y0: 327.,
            y1: 452.,
            heading: "TIME",
        },
        Card {
            x0: 292.,
            x1: 434.,
            y0: 327.,
            y1: 452.,
            heading: "BEATS",
        },
        Card {
            x0: 442.,
            x1: 584.,
            y0: 327.,
            y1: 452.,
            heading: "FEEDBACK",
        },
        Card {
            x0: 592.,
            x1: 734.,
            y0: 327.,
            y1: 452.,
            heading: "LEVEL",
        },
        Card {
            x0: 142.,
            x1: 284.,
            y0: 469.,
            y1: 594.,
            heading: "TIME",
        },
        Card {
            x0: 292.,
            x1: 434.,
            y0: 469.,
            y1: 594.,
            heading: "BEATS",
        },
        Card {
            x0: 442.,
            x1: 584.,
            y0: 469.,
            y1: 594.,
            heading: "FEEDBACK",
        },
        Card {
            x0: 592.,
            x1: 734.,
            y0: 469.,
            y1: 594.,
            heading: "LEVEL",
        },
    ],
    controls: &[
        Control::Knob {
            id: "low",
            cx: 79.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high",
            cx: 229.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mix",
            cx: 379.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "output",
            cx: 529.,
            cy: 85.,
            r: 24.,
            caption: "",
            bipolar: true,
        },
        Control::Choice {
            id: "sync",
            x: 620.,
            top: 66.,
            names: &["Off", "On"],
        },
        Control::Knob {
            id: "low_time",
            cx: 213.,
            cy: 242.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "low_beat",
            cx: 363.,
            cy: 242.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "low_feedback",
            cx: 513.,
            cy: 242.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "low_level",
            cx: 663.,
            cy: 242.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mid_time",
            cx: 213.,
            cy: 384.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mid_beat",
            cx: 363.,
            cy: 384.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mid_feedback",
            cx: 513.,
            cy: 384.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "mid_level",
            cx: 663.,
            cy: 384.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high_time",
            cx: 213.,
            cy: 526.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high_beat",
            cx: 363.,
            cy: 526.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high_feedback",
            cx: 513.,
            cy: 526.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
        Control::Knob {
            id: "high_level",
            cx: 663.,
            cy: 526.,
            r: 24.,
            caption: "",
            bipolar: false,
        },
    ],
    display: None,
    mark: Mark {
        x: 744.,
        y: 676.,
        anchor: 1.,
        stacked: false,
    },
    hint: None,
    labels: &[],
    text: multidelay_text,
    parse: multidelay_parse,
    sound: |_, _| unreachable!(),
    rings: true,
};
fn multidelay_text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{v} Hz"),
        1 => format!("{v} Hz"),
        2 => format!("{:.1}%", v as f64 / 10.),
        3 => format!("{:+.1} dB", v as f64 / 10.),
        4 => ["Off", "On"][v.clamp(0, 1) as usize].into(),
        5 => format!("{v} ms"),
        6 => [
            "1/16", "1/8 T", "1/8", "1/8 D", "1/4", "1/4 D", "1/2", "1/1",
        ][v.clamp(0, 7) as usize]
            .into(),
        7 => format!("{:.1}%", v as f64 / 10.),
        8 => {
            if v == -600 {
                "Off".into()
            } else {
                format!("{:+.1} dB", v as f64 / 10.)
            }
        }
        9 => format!("{v} ms"),
        10 => [
            "1/16", "1/8 T", "1/8", "1/8 D", "1/4", "1/4 D", "1/2", "1/1",
        ][v.clamp(0, 7) as usize]
            .into(),
        11 => format!("{:.1}%", v as f64 / 10.),
        12 => {
            if v == -600 {
                "Off".into()
            } else {
                format!("{:+.1} dB", v as f64 / 10.)
            }
        }
        13 => format!("{v} ms"),
        14 => [
            "1/16", "1/8 T", "1/8", "1/8 D", "1/4", "1/4 D", "1/2", "1/1",
        ][v.clamp(0, 7) as usize]
            .into(),
        15 => format!("{:.1}%", v as f64 / 10.),
        16 => {
            if v == -600 {
                "Off".into()
            } else {
                format!("{:+.1} dB", v as f64 / 10.)
            }
        }
        _ => String::new(),
    }
}
fn multidelay_parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    if k == 4 {
        return ["off", "on"]
            .iter()
            .position(|x| *x == s.to_lowercase())
            .map(|x| x as i32);
    }
    if k == 6 {
        return [
            "1/16", "1/8 t", "1/8", "1/8 d", "1/4", "1/4 d", "1/2", "1/1",
        ]
        .iter()
        .position(|x| *x == s.to_lowercase())
        .map(|x| x as i32);
    }
    if k == 8 && s.eq_ignore_ascii_case("off") {
        return Some(-600);
    }
    if k == 10 {
        return [
            "1/16", "1/8 t", "1/8", "1/8 d", "1/4", "1/4 d", "1/2", "1/1",
        ]
        .iter()
        .position(|x| *x == s.to_lowercase())
        .map(|x| x as i32);
    }
    if k == 12 && s.eq_ignore_ascii_case("off") {
        return Some(-600);
    }
    if k == 14 {
        return [
            "1/16", "1/8 t", "1/8", "1/8 d", "1/4", "1/4 d", "1/2", "1/1",
        ]
        .iter()
        .position(|x| *x == s.to_lowercase())
        .map(|x| x as i32);
    }
    if k == 16 && s.eq_ignore_ascii_case("off") {
        return Some(-600);
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
    let scale = if [2, 3, 7, 8, 11, 12, 15, 16].contains(&k) {
        10.
    } else {
        1.
    };
    let p = MULTIDELAY.settings.get(k)?;
    Some(((v * scale).round() as i32).clamp(p.range.0, p.range.1))
}

pub static RISER: Spec = Spec {
    name: "ML Riser",
    word: "RISER",
    about: "ML Riser. Based on NULL Riser by Christos Koulaxizis (MIT).",
    settings: &[Setting { id: "intensity", name: "Intensity", range: (0, 100), start: 0, magnet: None }],
    studio_settings: 1,
    version: None, any_version: false, fields: &[], tail: &[],
    size: (560, 270),
    cards: &[Card { x0: 16., x1: 206., y0: 28., y1: 200., heading: "INTENSITY" }],
    controls: &[Control::Knob { id: "intensity", cx: 111., cy: 106., r: 46., caption: "", bipolar: false }],
    display: None,
    mark: Mark { x: 542., y: 250., anchor: 1., stacked: false },
    hint: None, labels: &[],
    text: |_, v| format!("{v}%"),
    parse: |_, s| s.trim().trim_end_matches('%').trim().parse::<f64>().ok()
        .filter(|v| v.is_finite()).map(|v| v.round() as i32),
    sound: |_, _| unreachable!(), rings: true,
};
