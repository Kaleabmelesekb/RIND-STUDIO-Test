# ML Studio plugins

Twelve plugins from ML Studio, Michael Lane's fork of Lemon Studio (https://github.com/Dorfhaus/ML-Studio, GPL-3.0-or-later),
taken at commit 4acbe403382e4583800ff52a7a55d8d99b563163 and built into Honeydew like the Lemon Studio ones in
`../lemon-studio`: ML Tape, ML De-Esser, ML Frequency Shift, ML Meter, ML Multiband Delay, ML Phase, ML Pitch Shift,
ML Reference, ML Riser, ML Split, ML Tuner and ML Waveshaper.

`tools/vendor-ml-studio.py` copies a crate in. What differs from the fork:

- `ml-small-fx` keeps only the shared editor/parameter framework and Lemon Tape. The duplicates of Lemon Studio's own
  effects, Hall, the factory-preset tests and the state-edit undo are left out.
- The fork's `os-plugkit` `visibility` (always on), `LiveEdits::clear` and `os-dsp`'s `debess` are added to
  `../lemon-studio`; `ml-utility-core` has its own `Ring`.
- Not brought over yet: Lemon Chop, the Plasma and Master Assistant (JUCE) plugins, ML Drums, Vocal Tune (AGPL), and the
  plugins that need C++ (NAM amp, FluidSynth, convolver).

Honeydew's own additions in the same style (not from ML Studio): Rind Bitcrusher, Rind Exciter and Rind Spectral Compressor, in `ml-mix-core/src/extra.rs`.
