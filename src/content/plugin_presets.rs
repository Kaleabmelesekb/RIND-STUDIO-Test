//! Factory presets of the built-in Lemon plugins, `Presets/<plugin name>/[Category/]<Name>.flmpst`: the factory
//! presets Mango Studio (<https://github.com/apikun/Mango-Studio>, GPL-3.0-or-later, a fork of Lemon Studio) ships for
//! the same plugins, brought over from its `.ospreset` files (`plugin_presets.txt`, made by the script that is noted in
//! `THIRD_PARTY_NOTICES.md`). Each shows in its plugin's preset picker (`plugin_picker`) under the factory folders.
//!
//! A preset is one of two kinds, as in Mango Studio:
//! - **Named values**: a list `<parameter name> = <value>`, the value in the plugin's own range (a stepped parameter's
//!   step count from its minimum, any other 0..1). The module of the file holds them as params with the id
//!   [`NAMED_PARAM`] and the value as the default; a pick resolves them against the running plugin
//!   (`plugins::load_preset_state`), after every parameter went back to its default. A name the plugin lacks is skipped;
//!   the line `=0` (an empty name) is a preset that sets nothing, all defaults.
//! - **State**: the plugin's whole saved state, loaded as a user preset of the plugin is.

use std::sync::{Arc, OnceLock};

use super::{Entry, PRESETS};
use crate::flm;
use crate::song::ModuleInstance;
use crate::song::plugin::{NAMED_PARAM, PLUGIN_EFFECT, PLUGIN_INSTRUMENT, PluginParam, PluginRef};

/// The presets, one block each: `@@ <TAB> id <TAB> category <TAB> name`, then its `name=value` lines or a
/// `#state <base64>` line.
const DATA: &str = include_str!("plugin_presets.txt");

/// The built-in plugins that have presets: CLAP id, the plugin's name (the folder a user's presets are in, too) and
/// whether it is an instrument.
const PLUGINS: &[(&str, &str, bool)] = &[
    ("org.openstudio.lemon-chorus", "Lemon Chorus", false),
    (
        "org.openstudio.lemon-fast-distortion",
        "Lemon Fast Distortion",
        false,
    ),
    ("org.openstudio.lemon-filter", "Lemon Filter", false),
    ("org.openstudio.lemon-flanger", "Lemon Flanger", false),
    ("org.openstudio.lemon-limiter", "Lemon Limiter", false),
    (
        "org.openstudio.open-parametric-eq-2",
        "Lemon Parametric EQ",
        false,
    ),
    ("org.openstudio.lemon-phaser", "Lemon Phaser", false),
    ("org.openstudio.lemon-reverb", "Lemon Reverb", false),
    ("org.openstudio.open-sampler", "Lemon Sampler", true),
    (
        "org.openstudio.lemon-soft-clipper",
        "Lemon Soft Clipper",
        false,
    ),
    (
        "org.openstudio.lemon-stereo-enhancer",
        "Lemon Stereo Enhancer",
        false,
    ),
    (
        "org.openstudio.lemon-stereo-shaper",
        "Lemon Stereo Shaper",
        false,
    ),
    (
        "org.openstudio.lemon-transient-processor",
        "Lemon Transient Processor",
        false,
    ),
    ("org.openstudio.open-3x-osc", "Lemon Trio", true),
    ("org.openstudio.press", "Press", false),
    ("org.openstudio.pulp-filter", "Pulp Filter", false),
];

/// The factory folder (below [`PRESETS`]) of the built-in plugin with CLAP id `id`, if it has presets.
pub fn folder(id: &str) -> Option<&'static str> {
    (PLUGINS.iter())
        .find(|(p, ..)| p.eq_ignore_ascii_case(id))
        .map(|&(_, name, _)| name)
}

/// One preset read from [`DATA`].
struct Preset {
    id: &'static str,
    category: &'static str,
    name: &'static str,
    values: Vec<(&'static str, f32)>,
    state: Vec<u8>,
}

fn presets() -> &'static [Preset] {
    static ALL: OnceLock<Vec<Preset>> = OnceLock::new();
    ALL.get_or_init(|| parse(DATA))
}

fn parse(data: &'static str) -> Vec<Preset> {
    let mut all: Vec<Preset> = Vec::new();
    for line in data.lines() {
        if let Some(head) = line.strip_prefix("@@\t") {
            let mut f = head.split('\t');
            let (Some(id), Some(category), Some(name)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            all.push(Preset {
                id,
                category,
                name,
                values: Vec::new(),
                state: Vec::new(),
            });
        } else if let Some(text) = line.strip_prefix("#state ") {
            if let Some(last) = all.last_mut() {
                last.state = base64(text);
            }
        } else if let Some((name, value)) = line.rsplit_once('=')
            && let (Some(last), Ok(value)) = (all.last_mut(), value.trim().parse::<f32>())
        {
            last.values.push((name.trim(), value));
        }
    }
    all
}

/// Standard base64 (padding optional); characters outside it are skipped.
fn base64(text: &str) -> Vec<u8> {
    let (mut out, mut acc, mut bits) = (Vec::with_capacity(text.len() * 3 / 4), 0u32, 0);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// `preset`'s path below [`super::ROOT`].
fn rel(preset: &Preset) -> Option<String> {
    let folder = folder(preset.id)?;
    let category = match preset.category {
        "" => String::new(),
        category => format!("{category}/"),
    };
    Some(format!(
        "{PRESETS}/{folder}/{category}{}.{}",
        preset.name,
        crate::project_files::PRESET_EXT
    ))
}

/// The module `preset` is: its plugin's reference with the named values or the state.
fn module(preset: &Preset) -> ModuleInstance {
    let (name, instrument) = (PLUGINS.iter())
        .find(|(p, ..)| *p == preset.id)
        .map_or(("", false), |&(_, name, instrument)| (name, instrument));
    let type_id = if instrument {
        PLUGIN_INSTRUMENT
    } else {
        PLUGIN_EFFECT
    };
    let mut module = ModuleInstance::new(type_id, 0);
    module.file.plugin = Some(Box::new(PluginRef {
        format: "clap".into(),
        id: preset.id.into(),
        name: name.into(),
        vendor: String::new(),
        path: String::new(),
        state: Arc::from(preset.state.as_slice()),
        params: (preset.values.iter())
            .map(|&(name, value)| PluginParam {
                id: NAMED_PARAM,
                name: name.into(),
                default: value,
            })
            .collect(),
    }));
    module
}

pub(super) fn entries() -> Vec<Entry> {
    (presets().iter())
        .filter_map(|preset| {
            let bytes = move || flm::write_preset(&module(preset)).expect("a plugin preset writes");
            Some(Entry::new(rel(preset)?, bytes))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes() {
        assert_eq!(base64("aGVsbG8="), b"hello");
        assert_eq!(base64("aGVsbG8"), b"hello");
        assert_eq!(base64("AAEC/f7/"), [0, 1, 2, 253, 254, 255]);
    }

    #[test]
    fn every_preset_has_a_folder_and_values_or_a_state() {
        let all = presets();
        assert!(all.len() > 300);
        for p in all {
            assert!(folder(p.id).is_some(), "{} {}", p.id, p.name);
            assert!(
                p.values.is_empty() != p.state.is_empty(),
                "{} {}",
                p.id,
                p.name
            );
        }
    }

    #[test]
    fn a_preset_file_reads_back_as_its_plugins_module() {
        for p in presets() {
            let module = module(p);
            let bytes = flm::write_preset(&module).expect("writes");
            let back = flm::read_preset(&bytes, Some(module.type_id)).expect("reads");
            let plugin = back.file.plugin.expect("a plugin preset");
            assert_eq!(plugin.id, p.id);
            assert_eq!(plugin.params.len(), p.values.len(), "{}", p.name);
            assert_eq!(&*plugin.state, &p.state[..], "{}", p.name);
        }
    }
}
