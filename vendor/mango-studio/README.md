# Mango Studio plugins

Sixteen instruments from Mango Studio (https://github.com/apikun/Mango-Studio, apikun's fork of Lemon Studio,
GPL-3.0-or-later, `LICENSE` beside this file), taken at its `main` of 2026-11 and built into Honeydew like the Lemon
Studio ones in `../lemon-studio`: Lemon Kick, Lemon Boom, Lemon Bass, Lemon Acid, Lemon Module Drum, and Mango Pluck,
Mini, DX, Autogun, Drums, Keys, Dashboard, Flex, Monk, Slayer and Surface.

They share `lemon-small-gen` (the plugin, parameters and editor); each crate beside it is the plugin's own entry point.
Every file keeps its copyright and SPDX header. What differs from Mango Studio:

- The manifests are Honeydew's (`../../nih-plug`, `../lemon-studio`'s `os-*` crates, the `linked` and `embed` features).
- `lemon-small-gen/src/editor.rs` draws into Honeydew's own window (`embed`: the same changes `tools/sync-lemon-plugins.sh`
  makes to Lemon Studio's editors).
- The screenshot example and the `tiny-skia` dev-dependency are left out.

Mango's factory presets for the plugins Honeydew already had come separately: `src/content/plugin_presets.txt`
(`tools/gen-plugin-presets.py`).

Not brought over yet: Granulizer, Slicex, Wavetable, Beat, Beepmap, DirectWave, Master, Sfxr, Crusher, One Knob, Otter and
Twelve (they need more of Mango's own crates or changes to Lemon's small effects).
