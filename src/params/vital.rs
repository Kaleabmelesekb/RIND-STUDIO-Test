//! Vital ([`honeydew_audio::vital`]), Honeydew's own instrument: Matt Tytel's wavetable synthesizer (GPL-3.0-or-later),
//! its own sound engine. The params are Vital's own, by its names, ranges and units (`vital_table`, generated from the
//! engine's list by `tools/gen_vital_tables.py`), then the 16 modulation slots the card shows.

use super::ModuleSpec;
use super::vital_table::PARAMS;
use honeydew_audio::vital as v;

/// The module.
pub static MODULE: ModuleSpec = ModuleSpec::instrument(
    v::VITAL,
    "Vital",
    "Vital",
    &PARAMS,
    &["OSC", "FILTER", "ENV", "LFO", "FX", "MOD", "VOICE"],
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::ParamKind;
    use std::collections::HashSet;

    /// The registry and the engine agree on what param `n` is: Enable, then the engine's own table, in its order.
    #[test]
    fn the_params_are_the_engines_table_after_enable() {
        assert_eq!(PARAMS.len(), v::PARAMS);
        for (entry, spec) in v::TABLE.iter().zip(PARAMS.iter().skip(1)) {
            if matches!(entry.role, v::Role::Wavetable(_)) {
                continue; // a menu of the bridge's wavetables, which the engine's table does not count
            }
            match spec.kind {
                // Vital's two-item menus are the host's toggles.
                ParamKind::Toggle => assert!([0, 2].contains(&entry.menu), "{}", spec.name),
                ParamKind::Menu { items } => assert_eq!(items.len(), entry.menu, "{}", spec.name),
                _ => assert!(entry.menu == 0, "{} is not a menu", spec.name),
            }
        }
    }

    #[test]
    fn every_param_has_a_name_of_its_own() {
        let mut seen = HashSet::new();
        for spec in PARAMS.iter().skip(1) {
            assert!(!spec.name.is_empty());
            assert!(seen.insert(spec.name), "{} twice", spec.name);
        }
    }
}
