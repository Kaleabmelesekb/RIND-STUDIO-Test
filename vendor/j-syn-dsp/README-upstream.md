# J-SYN

A zero-allocation 64-voice polyphonic wavetable synthesizer VST3 and CLAP plugin written in Rust using `nih-plug` and `nih_plug_vizia`.

---

## Documentation

- 📖 **[User Guide (`docs/USER_GUIDE.md`)](./docs/USER_GUIDE.md)** — Walkthrough of the oscillators, filters, insert/master FX rack, modulation matrix, `MODSEQ` / `LFO` / `ENV` editors, preset/wavetable subfolders, themes, and shortcuts.

---

## Building & Installing the Plugin

### Building Locally from Source
```bash
# Run workspace unit and DSP tests
cargo test --workspace

# Build and bundle release VST3 and CLAP plugins
cargo xtask bundle j-syn --release
```

The compiled bundles are written to `target/bundled/j-syn.vst3` and `target/bundled/j-syn.clap`.

### Installing & Bypassing macOS Gatekeeper
On **macOS**, copy `j-syn.vst3` to `/Library/Audio/Plug-Ins/VST3/` (system-wide) or `~/Library/Audio/Plug-Ins/VST3/` (current user), and `j-syn.clap` to `/Library/Audio/Plug-Ins/CLAP/` or `~/Library/Audio/Plug-Ins/CLAP/`.

Because local builds are not notarized with an Apple Developer ID certificate, strip quarantine attributes and apply an ad-hoc signature if macOS Gatekeeper flags the bundles:

```bash
sudo cp -R target/bundled/j-syn.vst3 /Library/Audio/Plug-Ins/VST3/
sudo cp -R target/bundled/j-syn.clap /Library/Audio/Plug-Ins/CLAP/

sudo xattr -cr /Library/Audio/Plug-Ins/VST3/j-syn.vst3 /Library/Audio/Plug-Ins/CLAP/j-syn.clap
sudo codesign --force --deep --sign - /Library/Audio/Plug-Ins/VST3/j-syn.vst3
sudo codesign --force --deep --sign - /Library/Audio/Plug-Ins/CLAP/j-syn.clap
```

On **Windows**, copy `j-syn.vst3` to `C:\Program Files\Common Files\VST3\` and `j-syn.clap` to `C:\Program Files\Common Files\CLAP\`.
On **Linux**, copy them to `~/.vst3/` and `~/.clap/`.

---

## Synthesizer Highlights

1. **4 Wavetable Oscillators + Stereo Noise (`64` Voices) + Full MPE:**
   - `OSC 1`–`OSC 4` with `±48 st` pitch, phase, up to 8-voice unison (`UNI`, `DETUNE`, `SPREAD`), `WARP` (`None`, `B-`, `B+`, `SP`), cross-oscillator `PM`/`RM` with switchable warp-mod routing order, and custom `.wav` wavetable import (`2048`-sample frame slicing & mip-maps).
   - `Dirty` (2007-style table lookup) and `Clean` (band-limited mip-map interpolation) engine modes, voice `auto_sleep`, and per-voice stereo `White`/`Pink`/`Brown` noise.
   - **Full MPE Support:** Toggleable `MPE` mode with `1–96 st` (`48 st` default) Member Channel (`Ch 2–16`) pitch bend, per-voice Pressure (`AT`), `CC74` Slide (`SLD`), Master `Ch 1` broadcast, CLAP/VST3 poly expressions, and `1.5 ms` per-voice smoothing.
2. **Dual ZDF Multimode Filters & Insert FX:**
   - `FILTER 1` & `FILTER 2` with `LP`/`BP`/`HP`/`Notch`/`Peak`/`HP+Peak`/`HP+Notch`/`LP+Notch`/`LP+Peak`/`Peak+Peak`/`Notch+Notch` (`6/12/24/36/48 dB`), plus fixed `12 dB` `Bandreject`, `Scream`, `Daft`, and `Comb` modes with live post-modulation response scopes.
   - `INSERT FX` (`INS 1` & `INS 2`: `Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `Sine`, `Parabolic`, `SoftClip`, `Bitcrush`), `Serial`/`Parallel` filter routing, 4-way `Off | Bus A | Bus B | Sub` direct sends, per-filter bus routing (`Bus A | Bus B | A+B | Sub`), and feedback bus.
3. **Parallel / Serial Dual-Bus 8-Slot Master FX Rack & Sub Bus:**
   - Two independent 8-slot stereo buses (`Bus A` & `Bus B`, switchable `PARALLEL (A | B)` or `SERIAL (A -> B)`) plus a `SUB / DIRECT` bus (`LP CUTOFF`, `CLIP DRIVE`, `LEVEL`, passing clean Direct Out when bypassed).
   - Swappable `FX 1`–`FX 8` slots (`Empty`, `OTT`, `Distortion`, `Chorus`, `Phaser`, `Delay`, `Reverb`, `Clipper`, `Filter`, `Dispersion` 128-stage allpass, `Tube Res.`, `Utility`, and `Limiter` with `0.0 ms` transient-clip / `1–3 ms` S-curve lookahead) with `STACK 1..4`, floating `ZStack` scope overlays (`MAIN | EQ`/`LFO`/`ADV` sub-tabs, compact Filter mode/slope, Dispersion stages, `[TP]`, `[Ø L]`/`[Ø R]`), `f64`-precision `~2.5 Hz` DC blocker, optional `-0.1 dBFS` brickwall safety limiter, and stereo Peak/RMS meter + master scope. *(Note: the `DC Blocker` runs after the FX Rack and `MASTER` gain, so highpass phase-tilt on flat-topped/asymmetric waves can push peaks slightly over `0 dBFS` even with a `0 dB` Limiter/Clipper slot unless `Safety Limiter` is enabled.)*
4. **32-Slot Modulation Matrix, `MODSEQ` / `LFO` / `ENV` & Multi-Theme UI:**
   - Drag-and-drop `E1`–`E4`, `MS1`–`MS4`, `VEL`, `KEY`, `MW`, `AT`, `SLD`, and custom-named `M1`–`M8` macros onto knobs/pills, with radial modulation collars, right-click bypass/polarity/remove menus, and a full `MATRIX` table (`LIN`/`EXP`/`LOG`/`S-CURVE`/`STEP`, `AUX`, `UNI`/`BIP`).
   - 5 built-in themes (`Studio Amber`, `Arctic Slate`, `Midnight Violet`, `Phosphor`, `Daylight Studio`) + custom `.json` themes, persistent `UI SCALE` (`85%`–`200%`), adjustable `GEAR SIZE` in Settings, and collapsible subfolder category menus for presets and wavetables.

---

## Workspace Structure

```text
J-SYN/
├── .cargo/config.toml            # `cargo xtask` alias
├── .github/workflows/build.yml   # Tag-triggered macOS/Windows/Linux CI & Release workflow
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── j-syn-dsp/                # Pure, zero-allocation Rust DSP library
│   │   └── src/
│   │       ├── lib.rs            # Public DSP exports
│   │       ├── engine.rs         # 64-voice synth engine, DC blocker & safety limiter
│   │       ├── voice.rs          # Per-voice signal flow, routing & auto-sleep
│   │       ├── oscillator.rs     # Wavetable oscillator, unison, warp & PM/RM
│   │       ├── wavetable.rs      # Factory tables, .wav loader & band-limited mip-maps
│   │       ├── noise.rs          # Stereo White/Pink/Brown noise generator
│   │       ├── envelope.rs       # ADSR envelopes
│   │       ├── filter.rs         # Dual ZDF multimode, Scream, Daft, Comb & combo filters
│   │       ├── insert.rs         # Dual per-voice insert waveshapers & feedback bus
│   │       ├── fx.rs             # 8-slot serial Master FX Rack (STACK 1..4)
│   │       ├── mod_matrix.rs     # Lock-free 32-slot mod matrix & scope telemetry
│   │       ├── modseq.rs         # 4 hybrid 16-step MODSEQ / MSEG LFO modulators
│   │       ├── adaa.rs           # 2nd-order ADAA tanh waveshaper
│   │       └── oversample.rs     # 4x polyphase IIR half-band oversampler & RBJ biquads
│   └── j-syn/                    # "J-SYN" VST3/CLAP plugin, presets & Vizia GUI
├── docs/
│   └── USER_GUIDE.md             # Full interface & parameter guide
└── xtask/                        # `nih_plug_xtask` bundler
```

---

## License & Third-Party Attributions

Licensed under the **GNU General Public License v3.0 or later (GPL-3.0-or-later)**.

- **[`nih-plug`](https://github.com/robbert-vdh/nih-plug)** & **`nih_plug_vizia`** by Robbert van der Helm for the Rust VST3/CLAP plugin framework and Vizia GUI integration.
- **[`vitOTTx`](https://github.com/RustedYeast/vitOTTx)** (derived from Matt Tytel's Vital synth DSP) for 3-band Linkwitz-Riley OTT crossover and upward/downward envelope follower math.
- **[`CrushClone`](https://github.com/doxiedevops/CrushClone)** for Mech and Tube distortion waveshaping curves.
- **Scream** (`crates/j-syn-dsp/src/filter.rs`, `crates/j-syn-dsp/src/adaa.rs`) — MIT License, Copyright 2025 Cure Audio, exacoustics, Xynth Audio.
- **ADAA** (2nd-order Antiderivative Antialiasing `tanh`) — BSD 3-Clause License, Copyright (c) 2020 jatinchowdhury18.
- **Polylogarithm** (`li2`) — MIT License, Copyright (c) 2021 Alexander Voigt.

<details>
<summary>Full MIT & BSD-3-Clause License Texts (Scream, ADAA, Polylogarithm)</summary>

- **Scream** — MIT License  
  Copyright 2025 Cure Audio, exacoustics, Xynth Audio  
  Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:  
  The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.  
  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

- **ADAA** — BSD 3-Clause License  
  Copyright (c) 2020, jatinchowdhury18  
  All rights reserved.  
  Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:  
  1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.  
  2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.  
  3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.  
  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

- **Polylogarithm** (`li2`) — MIT License  
  Copyright (c) 2021 Alexander Voigt  
  Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:  
  The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.  
  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

</details>

---

## Disclaimer

This plugin was developed with heavy use of AI-assisted coding ("vibecoding"). While tested for audio safety and stability, use it at your own risk—always keep a limiter on your master bus when experimenting with extreme FM, feedback, or resonance settings. J-SYN is an independent open-source project and is not affiliated with or endorsed by any third-party software or hardware vendors.
