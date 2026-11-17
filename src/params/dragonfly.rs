//! The Dragonfly reverbs ([`honeydew_audio::dragonfly_fx`]), Honeydew's own effects after Michael Willis and Rob van
//! den Berg's Dragonfly Reverb plugins (GPL-3.0-or-later): each one's params, in the plugin's own order, names,
//! ranges and units.

use honeydew_audio::dragonfly_fx::{self as d, Law, Spec, early, hall, plate, room};

use super::{Display, ModuleSpec, ParamSpec as P};

/// One [`Spec`] as a registry param at `default`; a menu shows `items`.
const fn param(spec: &Spec, default: f32, items: &'static [&'static str]) -> P {
    match spec.law {
        Law::Linear => P::cont(
            spec.name,
            default,
            Display::linear(spec.min, spec.max, spec.decimals, spec.unit),
        ),
        Law::Menu => P::menu(spec.name, default, items),
    }
}

/// `Enable`, then `specs` (params 1 …) at `defaults`.
const fn table<const N: usize>(
    specs: &[Spec],
    defaults: &[f32; N],
    items: &'static [&'static str],
) -> [P; N] {
    assert!(specs.len() + 1 == N);
    let mut p = [P::ENABLE; N];
    let mut i = 0;
    while i < specs.len() {
        p[i + 1] = param(&specs[i], defaults[i + 1], items);
        i += 1;
    }
    p
}

static HALL_PARAMS: [P; hall::PARAMS] = table(&hall::SPECS, &hall::DEFAULTS, &[]);
static ROOM_PARAMS: [P; room::PARAMS] = table(&room::SPECS, &room::DEFAULTS, &[]);
static PLATE_PARAMS: [P; plate::PARAMS] =
    table(&plate::SPECS, &plate::DEFAULTS, &plate::ALGORITHMS);
static EARLY_PARAMS: [P; early::PARAMS] =
    table(&early::SPECS, &early::DEFAULTS, &early::PROGRAM_NAMES);

pub static HALL: ModuleSpec = ModuleSpec::effect(
    d::HALL,
    "Dragonfly Hall Reverb",
    "Dragonfly Hall Reverb",
    false,
    &HALL_PARAMS,
    &[],
);
pub static ROOM: ModuleSpec = ModuleSpec::effect(
    d::ROOM,
    "Dragonfly Room Reverb",
    "Dragonfly Room Reverb",
    false,
    &ROOM_PARAMS,
    &[],
);
pub static PLATE: ModuleSpec = ModuleSpec::effect(
    d::PLATE,
    "Dragonfly Plate Reverb",
    "Dragonfly Plate Reverb",
    false,
    &PLATE_PARAMS,
    &[],
);
pub static EARLY: ModuleSpec = ModuleSpec::effect(
    d::EARLY,
    "Dragonfly Early Reflections",
    "Dragonfly Early Reflections",
    false,
    &EARLY_PARAMS,
    &[],
);

/// The four modules.
pub static MODULES: [&ModuleSpec; 4] = [&HALL, &ROOM, &PLATE, &EARLY];
