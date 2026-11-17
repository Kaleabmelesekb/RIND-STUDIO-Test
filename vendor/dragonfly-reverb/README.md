# Dragonfly Reverb (vendored)

The DSP of Michael Willis and Rob van den Berg's [Dragonfly Reverb](https://github.com/michaelwillis/dragonfly-reverb)
(Hall, Room, Plate and Early Reflections), **unchanged**, as Rind Studio runs it. License: GPL-3.0-or-later
([`LICENSE`](LICENSE)); Freeverb3 under it is by Teru Kamogashira (GPL-2.0-or-later).

- `common/` and `plugins/*/` hold the upstream files as they are: `AbstractDSP.hpp`, `Param.hpp`, `fv3_config.h`, the
  Freeverb3 sources in `freeverb/`, and each plugin's `DSP.cpp`, `DSP.hpp`, `DistrhoPluginInfo.h` (its parameters,
  ranges and factory programs) and `Artwork.hpp`. Upstream commit: `faf0322e8d3d44cdf998e19a037aacba25231dcc`.
- Not taken: DPF, the plugin framework (formats, the OpenGL window), and the plugins' `Plugin.cpp` and `UI.cpp`.
  `shim/` replaces the two DPF headers the DSP files include (`d_isNotEqual`, `ScopedDenormalDisable`).
- `bridge.cpp` is ours: a C face (`new`, `set`, `run`, `mute`, `sample_rate`, `free`) for one plugin's DSP, built once per
  plugin. The four plugins all name their DSP class `DragonflyReverbDSP`, so `crates/audio/build.rs` builds each with
  its own name for it (`-DDragonflyReverbDSP=…`), which leaves the sources untouched.
- `crates/audio/build.rs` compiles all of it with the `cc` crate, for every target that has a C++ compiler, Android's
  NDK included. `crates/audio/src/dragonfly_fx.rs` calls it as four effects of the rack.
