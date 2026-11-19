# Vital, in Rind Studio

[Vital](https://github.com/mtytel/vital) is Matt Tytel's wavetable synthesizer, GPL-3.0-or-later. Rind Studio plays its
**own sound engine, unchanged**: the oscillators and wavetables, unison, voices, filters, envelopes, LFOs, random
sources, modulation matrix and effects are Vital's C++ from `src/synthesis` and `src/common`, copied here as they are.

What is not Vital's:

- `shim/JuceHeader.h` and `shim/load_save.h`: a small stand-in for the parts of JUCE the engine touches (strings, files,
  base64, a critical section), and for the three `LoadSave` helpers the wavetable code calls. Vital's window, its
  preset browser and its JUCE plugin shell are not used: Rind Studio has its own project format, audio device and card.
- `load_save_subset.cpp`: those three helpers, copied from `src/common/load_save.cpp` without change.
- `bridge.cpp`: the C face the Rust `vital` module calls (notes, parameters, the modulation matrix, `.vital` preset
  loading, rendering). It does what Vital's `SynthBase` does around the engine.
- `engine.cpp`: the unity translation unit, in the order Vital's own `synthesis.cpp` needs.
- The wavetables in `bridge.cpp`: Vital's factory wavetables are data files that are not part of its source, so the
  twelve here are built with Vital's own wavetable classes.
- `tables/`: Vital's parameter list, written from the running engine; `tools/gen_vital_tables.py` turns it into the
  Rust tables (`crates/audio/src/vital_table.rs`, `src/params/vital_table.rs`).

Third-party code Vital carries: nlohmann/json (MIT) and kissfft (BSD-3-Clause) in `third_party/`.

Vital's own window needs OpenGL and JUCE and does not run on Android: the studio's card stands in for it, with Vital's
own parameter names and ranges.
