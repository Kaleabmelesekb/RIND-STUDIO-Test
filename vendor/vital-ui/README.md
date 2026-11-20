# vital-ui

Vital's original interface, packaged for Android.

- `juce/` — JUCE 6.0.5 as Vital patches it, trimmed to the modules Vital's interface uses. JUCE is built for
  `JUCE_LINUX` on Android's bionic with a **windowless platform**: `juce_headless_Windowing.cpp` (a component peer
  with no window) and `juce_OpenGL_headless.h` (an EGL pbuffer + FBO; each frame is read back and handed to the app).
  `JUCE_HEADLESS_ES` makes the OpenGL paths GLES 3 (`#version 300 es`).
- `src/interface`, `src/common` — Vital's own sources (`shaders.cpp` has GLES precision fixes and prints a failing
  shader's source). `unity/` — the translation units.
- `rind_gui.{h,cpp}` — `RindSynth` (Vital's `SynthBase` on the app's shared engine) and the GUI thread: it owns the
  JUCE message loop, opens and closes the editor, takes pointer/wheel/key input, and publishes frames, changed
  parameters and modulation slots. `vendor/vital/bridge.cpp` exposes it as the C ABI `vital_gui_*` that
  `crates/audio/src/vital.rs` (`vital::gui`) calls; `src/ui/rack/vital_card.rs` is the card.
- One interface at a time (one GL context): the module that opened last owns it.
- Built only for Android (`crates/audio/build.rs`, `vital_ui()`); elsewhere the engine-only build is used.
- Checks: `tools/vital_ui_probe.sh` and the "Vital UI probe" workflow compile and link everything for both ABIs;
  `tools/vital_ui_sandbox` renders the interface with Mesa EGL on a desktop.

Licences: Vital and JUCE are GPL-3.0; FreeType (`vendor/freetype`) is FTL. See `THIRD_PARTY_NOTICES.md`.
