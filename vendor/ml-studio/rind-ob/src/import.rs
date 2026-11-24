// SPDX-License-Identifier: GPL-3.0-or-later
//! Patch files Rind OB opens: OB-Xf's own `.fxp` patches, and OB-Xd's `.fxp` and `.fxb` (the original OB-Xd by Vadim
//! Filatov and DiscoDSP's 2.x, `<discoDSP>` and `<Datsounds>`), converted the way OB-Xf's own OB-Xd importer
//! (`state/ObxdImporter.cpp`) does: its tables are followed here value for value.
//!
//! All of them are a VST program chunk (`CcnK`, `FPCh` for a patch, `FBCh` for a bank) holding JUCE's binary XML
//! (`VC2!`, a length, the XML text).
use crate::settings::{COUNT, KINDS, SETTINGS, XML};

/// A patch read from a file: every setting as Rind OB stores it.
#[derive(Clone)]
pub struct Imported {
    pub name: String,
    pub author: String,
    pub values: Vec<u16>,
}

struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
}
impl Tag {
    fn get(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn number(&self, key: &str) -> Option<f32> {
        self.get(key)?.trim().parse::<f32>().ok().filter(|v| v.is_finite())
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// The start tags of an XML text, in order (comments, declarations and end tags left out).
fn tags(xml: &str) -> Vec<Tag> {
    let b = xml.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &xml[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |n| n + 3);
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") || rest.starts_with("</") {
            i += rest.find('>').map_or(rest.len(), |n| n + 1);
            continue;
        }
        i += 1;
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'/' && b[i] != b'>' {
            i += 1;
        }
        let mut tag = Tag { name: xml[start..i].to_string(), attrs: Vec::new() };
        loop {
            while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
                i += 1;
            }
            if i >= b.len() || b[i] == b'>' {
                i += 1;
                break;
            }
            let k = i;
            while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                i += 1;
            }
            let key = xml[k..i].to_string();
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && b[i] == b'=' {
                i += 1;
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                    let q = b[i];
                    let v = i + 1;
                    i = v;
                    while i < b.len() && b[i] != q {
                        i += 1;
                    }
                    tag.attrs.push((key, unescape(&xml[v..i.min(b.len())])));
                    i += 1;
                }
            } else if key.is_empty() {
                i += 1;
            }
        }
        out.push(tag);
    }
    out
}

/// The XML text of a program or bank file: JUCE's binary XML (`VC2!`, a little-endian length, the text) or plain XML.
fn xml_of(b: &[u8]) -> Option<String> {
    if let Some(at) = b.windows(4).position(|w| w == b"VC2!") {
        let body = b.get(at + 8..)?;
        let len = u32::from_le_bytes(b.get(at + 4..at + 8)?.try_into().ok()?) as usize;
        let body = &body[..len.min(body.len())];
        return Some(String::from_utf8_lossy(body).trim_end_matches('\0').to_string());
    }
    let at = b.iter().position(|&c| c == b'<')?;
    Some(String::from_utf8_lossy(&b[at..]).trim_end_matches('\0').to_string())
}

pub fn index_of(id: &str) -> Option<usize> {
    XML.iter().position(|x| &x[..x.len() - 1] == id.as_bytes())
}

/// The stored value (0..=1000) of a normalised value, as the engine reads it: what `gen.py` does for the factory patches.
pub fn snap(kind: u8, v: f32) -> i32 {
    let v = f64::from(v).clamp(0., 1.);
    match kind {
        b'b' => if v >= 0.5 { 1000 } else { 0 },
        b't' => (v * 2.).round() as i32 * 500,
        b'v' => crate::voices_value((1 + (v * 32.) as i32).min(32)),
        b'l' => [100, 400, 700, 1000][((v * 3.) as usize).min(3)],
        b'p' => [100, 500, 1000][((v * 2.) as usize).min(2)],
        b'x' => ((v * 14.).round() / 14. * 1000.).round() as i32,
        b'n' => if v < 1. / 3. { 100 } else if v < 2. / 3. { 500 } else { 900 },
        _ => (v * 1000.).round() as i32,
    }
}

/// The settings at their starting values, as normalised values.
fn defaults() -> [f32; COUNT] {
    std::array::from_fn(|k| SETTINGS[k].start as f32 / 1000.)
}

fn finish(v: &[f32; COUNT], name: &str, author: &str) -> Imported {
    Imported {
        name: name.trim().to_string(),
        author: author.trim().to_string(),
        values: (0..COUNT).map(|k| snap(KINDS[k], v[k]) as u16).collect(),
    }
}

/// An OB-Xf program: its attributes are the settings by OB-Xf's names.
fn from_obxf(t: &Tag, version: &str) -> Imported {
    let mut v = defaults();
    for (k, x) in XML.iter().enumerate() {
        if let Some(n) = t.number(std::str::from_utf8(&x[..x.len() - 1]).unwrap_or("")) {
            v[k] = n;
        }
    }
    // Patches from before OB-Xf 1.1.1 hold the unison voices as 0..7.
    if version < "2025-12-13" {
        if let (Some(k), Some(_)) = (index_of("UnisonVoices"), t.number("UnisonVoices")) {
            v[k] = (v[k] * 7.).round().clamp(0., 7.) / 31.;
        }
    }
    finish(&v, t.get("programName").unwrap_or("Init"), t.get("author").unwrap_or(""))
}

// OB-Xd's parameter numbers, as they are written to disk.
mod xd {
    pub const VOLUME: usize = 2;
    pub const VOICE_COUNT: usize = 3;
    pub const TUNE: usize = 4;
    pub const OCTAVE: usize = 5;
    pub const BENDRANGE: usize = 6;
    pub const BENDOSC2: usize = 7;
    pub const LEGATOMODE: usize = 8;
    pub const BENDLFORATE: usize = 9;
    pub const VFLTENV: usize = 10;
    pub const VAMPENV: usize = 11;
    pub const ASPLAYEDALLOCATION: usize = 12;
    pub const PORTAMENTO: usize = 13;
    pub const UNISON: usize = 14;
    pub const UDET: usize = 15;
    pub const OSC2_DET: usize = 16;
    pub const LFOFREQ: usize = 17;
    pub const LFOSINWAVE: usize = 18;
    pub const LFOSQUAREWAVE: usize = 19;
    pub const LFOSHWAVE: usize = 20;
    pub const LFO1AMT: usize = 21;
    pub const LFO2AMT: usize = 22;
    pub const LFOOSC1: usize = 23;
    pub const LFOOSC2: usize = 24;
    pub const LFOFILTER: usize = 25;
    pub const LFOPW1: usize = 26;
    pub const LFOPW2: usize = 27;
    pub const OSC2HS: usize = 28;
    pub const XMOD: usize = 29;
    pub const OSC1P: usize = 30;
    pub const OSC2P: usize = 31;
    pub const OSC_QUANTIZE: usize = 32;
    pub const OSC1SAW: usize = 33;
    pub const OSC1PUL: usize = 34;
    pub const OSC2SAW: usize = 35;
    pub const OSC2PUL: usize = 36;
    pub const PW: usize = 37;
    pub const BRIGHTNESS: usize = 38;
    pub const ENVPITCH: usize = 39;
    pub const OSC1MIX: usize = 40;
    pub const OSC2MIX: usize = 41;
    pub const NOISEMIX: usize = 42;
    pub const FLT_KF: usize = 43;
    pub const CUTOFF: usize = 44;
    pub const RESONANCE: usize = 45;
    pub const MULTIMODE: usize = 46;
    pub const FILTER_WARM: usize = 47;
    pub const BANDPASS: usize = 48;
    pub const FOURPOLE: usize = 49;
    pub const ENVELOPE_AMT: usize = 50;
    pub const LATK: usize = 51;
    pub const LDEC: usize = 52;
    pub const LSUS: usize = 53;
    pub const LREL: usize = 54;
    pub const FATK: usize = 55;
    pub const FDEC: usize = 56;
    pub const FSUS: usize = 57;
    pub const FREL: usize = 58;
    pub const ENVDER: usize = 59;
    pub const FILTERDER: usize = 60;
    pub const PORTADER: usize = 61;
    pub const PAN1: usize = 62;
    pub const LFO_SYNC: usize = 72;
    pub const PW_ENV: usize = 73;
    pub const PW_ENV_BOTH: usize = 74;
    pub const ENV_PITCH_BOTH: usize = 75;
    pub const FENV_INVERT: usize = 76;
    pub const PW_OSC2_OFS: usize = 77;
    pub const LEVEL_DIF: usize = 78;
    pub const SELF_OSC_PUSH: usize = 79;
}

/// OB-Xd's `logsc`, and the inverses of its and OB-Xf's scales (OB-Xf's importer's own).
fn logsc(p: f32, lo: f32, hi: f32, rolloff: f32) -> f32 {
    (((p * (rolloff + 1.).ln()).exp() - 1.) / rolloff) * (hi - lo) + lo
}
fn inv_linsc(y: f32, lo: f32, hi: f32) -> f32 {
    if hi == lo { 0. } else { ((y - lo) / (hi - lo)).clamp(0., 1.) }
}
fn inv_logsc(y: f32, lo: f32, hi: f32, rolloff: f32) -> f32 {
    if hi == lo {
        return 0.;
    }
    let t = rolloff * (y - lo) / (hi - lo) + 1.;
    if t <= 0. { 0. } else { (t.ln() / (rolloff + 1.).ln()).clamp(0., 1.) }
}

/// An OB-Xd program (`Val_<n>` attributes, or the bare number in the oldest files).
fn from_obxd(t: &Tag) -> Imported {
    use xd::*;
    let mut v = defaults();
    let val = |n: usize| t.number(&format!("Val_{n}")).or_else(|| t.number(&n.to_string())).unwrap_or(0.);
    let mut set = |id: &str, x: f32| {
        if let Some(k) = index_of(id) {
            v[k] = x;
        }
    };
    for (id, x) in [
        ("Osc2Keytrack", 1.), ("EnvToPitchInvert", 0.), ("EnvToPWInvert", 0.), ("RingModMix", 0.), ("NoiseColor", 0.),
        ("Filter4PoleXpander", 0.), ("FilterXpanderMode", 0.), ("AmpEnvAttackCurve", 0.), ("FilterEnvAttackCurve", 0.),
        ("LFO1PW", 0.), ("LFO1ToVolume", 0.), ("VibratoWave", 0.), ("NotePriority", 0.), ("EnvLegatoMode", 0.),
    ] {
        set(id, x);
    }
    let new_format = t.get("voiceCount").is_some();
    let sync = val(LFO_SYNC) > 0.5;
    let step = val(OSC_QUANTIZE) > 0.5;
    // Master.
    let transpose = ((val(OCTAVE) * 4.).round() + 1.) as i32;
    set("Volume", val(VOLUME));
    set("Tune", val(TUNE));
    set("Transpose", transpose.clamp(0, 4) as f32 * 0.25);
    // Global.
    let vc = if new_format { val(VOICE_COUNT) } else { val(VOICE_COUNT) * 0.25 };
    let voices = (((vc * 31.).round() - 0.000_001) as i32 + 1).clamp(1, 32);
    set("Polyphony", ((voices - 1) as f32 + 0.5) / 32.);
    set("HQMode", val(FILTER_WARM));
    let bend = (if val(BENDRANGE) > 0.5 { 12. } else { 2. }) / 48.;
    set("PitchBendUp", bend);
    set("PitchBendDown", bend);
    set("BendOsc2Only", val(BENDOSC2));
    set("VibratoRate", inv_linsc(logsc(val(BENDLFORATE), 3., 10., 19.), 2., 12.));
    set("Portamento", val(PORTAMENTO));
    set("Unison", val(UNISON));
    set("UnisonDetune", inv_logsc(logsc(val(UDET), 0.001, 0.90, 19.), 0.001, 1.0, 19.));
    set("EnvLegatoMode", val(LEGATOMODE));
    set("NotePriority", if val(ASPLAYEDALLOCATION) > 0.5 { 0. } else { 0.5 });
    // Oscillators.
    let (mut p1, mut p2) = (val(OSC1P) * 48., val(OSC2P) * 48.);
    if p1 <= 36. && p2 <= 36. && transpose > 4 {
        p1 += 12.;
        p2 += 12.;
    }
    let pitch = |p: f32| if step { p.trunc() / 48. } else { p / 48. };
    set("Osc1Pitch", pitch(p1));
    set("Osc2Pitch", pitch(p2));
    set("Osc2Detune", val(OSC2_DET));
    set("Osc1SawWave", val(OSC1SAW));
    set("Osc1PulseWave", val(OSC1PUL));
    set("Osc2SawWave", val(OSC2SAW));
    set("Osc2PulseWave", val(OSC2PUL));
    set("OscSync", val(OSC2HS));
    set("OscCrossmod", val(XMOD) * 0.5);
    set("OscPW", val(PW));
    set("Osc2PWOffset", val(PW_OSC2_OFS) * (0.75 / 0.95));
    set("EnvToPWAmount", val(PW_ENV) * (0.85 / 1.055_555_555_5));
    set("EnvToPWBothOscs", val(PW_ENV_BOTH));
    set("EnvToPitchAmount", val(ENVPITCH) * (36. / 40.));
    set("EnvToPitchBothOscs", val(ENV_PITCH_BOTH));
    set("OscBrightness", val(BRIGHTNESS));
    // Mixer.
    set("Osc1Mix", val(OSC1MIX));
    set("Osc2Mix", val(OSC2MIX));
    set("NoiseMix", logsc(val(NOISEMIX), 0., 1., 35.));
    // Filter.
    set("FilterCutoff", val(CUTOFF));
    set("FilterResonance", val(RESONANCE));
    set("FilterMode", val(MULTIMODE));
    set("Filter2PoleBPBlend", val(BANDPASS));
    set("Filter4PoleMode", val(FOURPOLE));
    set("Filter2PolePush", val(SELF_OSC_PUSH));
    set("FilterKeyFollow", val(FLT_KF));
    set("FilterEnvAmount", val(ENVELOPE_AMT));
    set("FilterEnvInvert", val(FENV_INVERT));
    // Envelopes: OB-Xf's attack is a third of OB-Xd's.
    let attack = |x: f32, lo: f32| inv_logsc(logsc(x, lo, 60000., 900.) / 3., lo, 60000., 900.);
    set("AmpEnvAttack", attack(val(LATK), 4.));
    set("AmpEnvDecay", val(LDEC));
    set("AmpEnvSustain", val(LSUS));
    set("AmpEnvRelease", val(LREL));
    set("VelToAmpEnv", val(VAMPENV));
    set("FilterEnvAttack", attack(val(FATK), 1.));
    set("FilterEnvDecay", val(FDEC));
    set("FilterEnvSustain", val(FSUS));
    set("FilterEnvRelease", val(FREL));
    set("VelToFilterEnv", val(VFLTENV));
    // LFO.
    set("LFO1TempoSync", if sync { 1. } else { 0. });
    if sync {
        const TO_XF: [f32; 9] = [1., 4., 5., 7., 10., 11., 13., 15., 16.];
        set("LFO1Rate", TO_XF[((val(LFOFREQ) * 8.) as usize).min(8)] / 20.);
    } else {
        set("LFO1Rate", inv_logsc(logsc(val(LFOFREQ), 0., 50., 120.), 0., 250., 3775.));
    }
    let blend = |x: f32| if x >= 0.5 { 0. } else { 0.5 };
    let tri = |x: f32| if x >= 0.5 { 0.5 } else { 0. };
    set("LFO1Wave1", blend(val(LFOSINWAVE)));
    set("LFO1Wave2", blend(val(LFOSQUAREWAVE)));
    set("LFO1Wave3", blend(val(LFOSHWAVE)));
    set("LFO1ModAmount1", val(LFO1AMT));
    set("LFO1ModAmount2", val(LFO2AMT));
    set("LFO1ToOsc1Pitch", tri(val(LFOOSC1)));
    set("LFO1ToOsc2Pitch", tri(val(LFOOSC2)));
    set("LFO1ToFilterCutoff", tri(val(LFOFILTER)));
    set("LFO1ToOsc1PW", tri(val(LFOPW1)));
    set("LFO1ToOsc2PW", tri(val(LFOPW2)));
    // Voice variation and pan.
    set("PortamentoSlop", val(PORTADER));
    set("FilterSlop", val(FILTERDER));
    set("EnvelopeSlop", val(ENVDER));
    set("LevelSlop", val(LEVEL_DIF));
    for n in 0..8 {
        set(&format!("PanVoice{}", n + 1), val(PAN1 + n));
    }
    finish(&v, t.get("programName").unwrap_or("Init"), "")
}

/// The patches of an OB-Xd or OB-Xf `.fxp` or `.fxb` (up to 128 of a bank).
pub fn parse(bytes: &[u8]) -> Result<Vec<Imported>, &'static str> {
    const NOT: &str = "NOT AN OB-XD OR OB-XF FILE";
    let xml = xml_of(bytes).ok_or(NOT)?;
    let all = tags(&xml);
    let root = all.first().ok_or(NOT)?;
    let programs = || all.iter().filter(|t| t.name == "program").take(128);
    let out: Vec<Imported> = match root.name.as_str() {
        "OB-Xf" => {
            let version = root.get("ob-xf_version").unwrap_or("").to_string();
            if programs().next().is_some() {
                programs().map(|t| from_obxf(t, t.get("ob-xf_version").unwrap_or(&version))).collect()
            } else {
                vec![from_obxf(root, &version)]
            }
        }
        "discoDSP" | "Datsounds" => {
            if programs().next().is_some() {
                // A bank's empty slots are called "Default".
                programs().filter(|t| !t.get("programName").unwrap_or("Default").trim().eq_ignore_ascii_case("default")).map(from_obxd).collect()
            } else {
                vec![from_obxd(root)]
            }
        }
        _ => return Err(NOT),
    };
    if out.is_empty() { Err("NO PATCHES IN THE FILE") } else { Ok(out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obxd(extra: &str) -> Vec<u8> {
        let xml = format!(
            "<?xml version=\"1.0\"?><discoDSP><programs><program programName=\"Lead\" voiceCount=\"1.0\" Val_2=\"0.5\" Val_3=\"0.25\" {extra}/><program programName=\"Default\"/></programs></discoDSP>"
        );
        let mut b = b"CcnK\0\0\0\0FBCh".to_vec();
        b.extend_from_slice(b"\0\0\0\x01Obxd");
        b.extend_from_slice(b"VC2!");
        b.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        b.extend_from_slice(xml.as_bytes());
        b
    }

    #[test]
    fn reads_an_obxd_bank() {
        let got = parse(&obxd("Val_44=\"0.75\"")).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "Lead");
        assert_eq!(got[0].values.len(), COUNT);
        assert_eq!(got[0].values[index_of("FilterCutoff").unwrap()], 750);
        assert_eq!(got[0].values[index_of("Volume").unwrap()], 500);
    }

    #[test]
    fn reads_an_obxf_patch() {
        let xml = "<?xml version=\"1.0\" ?><OB-Xf ob-xf_version=\"2025-12-13\" voiceCount=\"1\" Volume=\"0.3\" Polyphony=\"0.2285\" programName=\"A &amp; B\"/>";
        let mut b = b"CcnK".to_vec();
        b.extend_from_slice(b"VC2!");
        b.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        b.extend_from_slice(xml.as_bytes());
        let got = parse(&b).unwrap();
        assert_eq!(got[0].name, "A & B");
        assert_eq!(got[0].values[index_of("Volume").unwrap()], 300);
        assert_eq!(crate::voices(i32::from(got[0].values[index_of("Polyphony").unwrap()])), 8);
    }

    #[test]
    fn refuses_other_files() {
        assert!(parse(b"hello").is_err());
        assert!(parse(b"<other a=\"1\"/>").is_err());
    }
}
