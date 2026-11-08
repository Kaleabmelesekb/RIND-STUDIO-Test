# Honeydew: architecture audit and roadmap

Status: audit only. No product code was changed for this document.
Method: code reading by four parallel read-only passes (data model and persistence, audio engine, UI, platform and
tests). Nothing here was built or run. Claims marked (unverified) were only partly traced and need a check before
work starts on them.

## 0. What Honeydew is today

Honeydew is a clean-room, FL-Studio-Mobile-style DAW (~286k lines of Rust outside `vendor/`): egui/eframe UI, a cpal
audio backend, its own engine, and `.flm` as the native project format (FLP import/export on top). Its data model,
layout, editors and file format are shaped by that heritage. The directive asks for an original DAW with an
Ableton/Bitwig/Cubase-grade workflow. The consequence that drives everything below:

* **Compatibility with `.flm` is a constraint.** Honeydew round-trips unknown chunks and keeps its own data in
  `HDxx` chunks. New concepts (groups, markers, takes, time-signature maps, launcher scenes) must be stored in
  Honeydew-owned chunks, and must degrade safely when a file is opened by a version that does not know them.
  Decision needed from the project owner: keep `.flm` as the native format (recommended), or move to a new native
  format with `.flm` as import/export only.
* **Touch-first, one-editor-under-the-playlist UI.** A desktop workspace (docking, several editors visible) is a new
  UI layer, not a restyle.

## 1. Capability inventory (evidence in section 9)

Implemented and working per the code: stable channel ids; send/return buses by routing any channel to any channel;
sidechain; parallel mixer stages (opt-in multicore); lock-free meters; peak/RMS/true-peak/LUFS/spectrum/vectorscope
(Analyzer module); input recording with latency compensation and takes as separate WAV files; WSOLA time-stretch and
pitch-shift; transient detection (slicer); offline render with WAV/FLAC/Ogg/MP3, stems, normalize, trim; snapshot
undo/redo with coalescing; project versions; single-slot autosave and crash recovery; ZIP project packaging;
FLP/MIDI import and export; piano roll with velocity/pan/slide lanes, quantize, humanize, legato, scale snap, ghost
notes, chord editor; drum and step editors; automation editor with Linear/Square/Smooth/Stairs shapes; nine themes
plus a theme creator; JSON keymaps; mouse tooltips; about 4,500 unit tests in `src` and 1,180 in `crates`.

## 2. Missing functionality (classified)

| Feature | Status | Notes |
|---|---|---|
| Group/folder tracks, nesting | Missing | no concept in `song/`; routing by sends only |
| Dedicated group buses | Partial | any channel can be a send target |
| Pre-fader sends | Missing | sends are post-fader, post-pan |
| Plugin delay compensation | Missing | `latency()` exposed by host, not consumed |
| Markers, locators, sections | Missing | |
| Punch in/out | Missing | loop region only |
| Takes / comping | Partial | takes are separate files, no lanes or comp UI |
| Tempo map | Partial | master automation lane, Honeydew-only; one time signature per song |
| Time-signature changes | Missing | SMF `FF 58` silently dropped on import and export |
| Clip split/consolidate in playlist | Partial | split exists as region operation; consolidate unverified |
| Crossfades | Missing | fades only; sample edits are destructive |
| Beat/tempo detection | Missing | transient detection exists |
| Freeze / bounce track | Missing | offline render exists |
| Clip launcher | Missing | `Clip` ids are per-lane, no scene concept |
| Expression lanes (CC, pressure) in piano roll | Partial | CC exists as automation tracks only |
| MPE, probability, groove templates, strum | Missing / unverified | |
| Docking, saved workspaces, panel rearrange | Missing | |
| Command palette, shortcut rebinding UI | Missing | keymap JSON loads; no UI |
| Light/dark switch | Partial | nine themes, no mode pairing |
| Rolling autosave, `.bak`, backups | Partial | single slot; manual versions only |
| Relink missing samples (`.flm`) | Partial/unverified | locate flow seen for FLP |
| Project migration | Missing | newer-file prompt only; FLM2 refused |
| Reference-track A/B, export analysis | Missing | analyzer module exists |
| Device hot-plug handling (desktop) | Unverified | only a `failed` flag seen |
| Input/output clock drift handling | Missing | no resampler, hard skip on lag |

## 3. Preserve

The `Channel` id scheme and `delete_channel` reference repair; the routing graph (Kahn stages, cycle rejection);
sidechain keys; the audio-thread discipline (command queue, `ReplaceSong` with retired-engine return, seqlock
meters, streamer thread, allocation test `rt_alloc`); the FLM reader/writer with unknown-chunk preservation;
`history.rs` snapshot sharing; atomic file writes; the pure layout solver and offscreen UI tests; the Analyzer;
render pipeline.

## 4. Improve

* Undo: add loop range and (new) markers/tempo map as edits; measure the "whole channel copy per edit" cost on dense
  songs before redesigning.
* Autosave: rotate N generations with checksums so one corrupt file cannot lose recovery.
* Plugin boundary: `try_lock` spin of up to 3 ms inside the callback is a dropout source; replace with a lock-free
  handoff once PDC work touches that code.
* Input path: drift between separate devices; device-change recovery on desktop.
* Clip ids unique only within a lane: blocks cross-lane features (launcher, comping); introduce a project-wide clip
  id in a Honeydew chunk, mapped on load.

## 5. Architectural weaknesses

1. Giant files: `main_screen.rs` 6937, `app.rs` 6460, `clips.rs` 5256 lines, `crates/audio/src/lib.rs` 3297,
   `audio_engine/mod.rs` 2962, `sample_cache.rs` 3824. Any large UI feature should first extract from these.
2. Views edit `Song` directly and then call `song_edited()` etc. There is no command layer, so "all edits undoable"
   depends on every call site.
3. Design intent is documented only in comments and milestone tags; there is no architecture document.
4. Honeydew-only data (tempo lane, sidechain, `HDHd`) has no migration path.

## 6. UI/UX assessment

Deliberate FL-Mobile look: flat, large targets, one editor below the playlist, modal popups, `U`-based scaling with a
desktop 2x boost. It will read as a stretched mobile app on a large screen. A workspace system needs: a panel
host with splitters and collapse, a saved-layout store (new settings file), a reusable context-menu and tooltip
service (touch long-press equivalent), and a design-token module that generalises `theme.rs` (spacing, borders,
states) rather than only colours.

## 7. Platform limits

Android: cpal/AAudio, only built-in Lemon plugins run, no on-device tests. Web: stretch/pitch only resample, no
plugins. iOS experimental. macOS arm64 only. FreeBSD has no MIDI input. Windows and macOS CI runs a single test.
Android is a first-class target, so every workspace feature needs a touch path; desktop-only docking must collapse
to the existing one-editor layout on small screens.

## 8. Dependency map (what blocks what)

```
stable project-wide ids ──► groups ──► group routing/automation/mute-solo
        │                       └────► launcher (clips independent of lanes)
        ├──► markers/locators ──► punch ──► takes/comping lanes
        └──► tempo + time-signature map ──► SMF fidelity, audio-clip tempo analysis
command layer over Song ──► complete undo ──► every feature above
panel host + design tokens ──► workspaces ──► command palette, shortcut UI
pre-fader sends, PDC ──► routing visualisation, freeze/bounce
```

## 9. Evidence index

Model and persistence: `src/song/{mod,channel,lane,points,tempo}.rs`, `src/history.rs`, `src/versions.rs`,
`src/project_files/{mod,export,archive}.rs`, `src/flm/`, `src/smf/{codec,import,export}.rs`.
Audio: `crates/audio/src/{device,mixer,stream,input}.rs`, `src/audio_engine/{mod,routing,parallel}.rs`,
`src/audio_link/`, `src/recording/`, `src/plugins/engine.rs`, `src/render.rs`, `src/sample_cache.rs`,
`analysis/analyzer.rs`.
UI: `src/ui/views/mod.rs`, `src/theme.rs`, `src/metrics.rs`, `src/keymap.rs`, `src/ui/{layout,split,clips,mixer}.rs`,
`src/ui/piano_roll/`, `src/ui/automation/`.
Platform: `.github/workflows/*.yml`, `Cargo.toml`, `README.md`.

## 10. Roadmap

Each milestone is independently reviewable, ends green on CI (fmt, clippy `-D warnings`, `cargo test --workspace`,
wasm check, Android APK) and adds tests for new behaviour including save/load and undo/redo.

| # | Milestone | Scope | Tests required |
|---|---|---|---|
| R0 | Foundations | Architecture doc; extract a project-wide id type and a clip-id map; command/undo coverage audit of every `song_edited()` call; rolling autosave with checksums | id stability across delete/reorder; recovery from corrupt newest autosave; undo round-trip property test |
| R1 | Markers, locators, sections, punch region | New Honeydew chunk; undoable; SMF marker import/export | serialize/restore, undo, SMF round trip |
| R2 | Tempo and time-signature map | Replace single `TimeSig` with a map; SMF `FF 58`; ruler and grid follow bars | SMF fidelity, playback position math, old files still load |
| R3 | Groups | Group channel kind with parent ids, collapse, group mute/solo, group routing, automation; nesting | create/move/delete sequences, undo, cyclic nesting rejected, old-version open |
| R4 | Workspace host | Panel host with splitters, collapse, saved layouts, restore default; phone fallback to the current layout; design-token module | layout solver unit tests, offscreen UI tests, persisted layout round trip |
| R5 | Mixer and routing | Pre-fader sends, routing view, mixer snapshots/copy-paste, channel presets, clip indicators; PDC with plugin latency | engine tests with latency plugin, null tests, `rt_alloc` unchanged |
| R6 | Piano-roll expression | CC/pressure/pitch-bend lanes in roll, probability, groove/swing templates, strum | note-data round trip including SMF |
| R7 | Recording and comping | Punch recording, take lanes, comp tool, crossfades, non-destructive audio edits | latency alignment test, loop-record take assignment, undo |
| R8 | Automation and tempo curves | Bezier shape, parameter linking, record automation with smoothing against zipper noise | audio glitch test on step changes |
| R9 | Freeze/bounce, background waveform work, CPU/dropout monitor | | render equals live null test |
| R10 | Clip launcher | Shared clip data, scenes, quantised launch, record to arrangement | transport/launch timing tests |
| R11 | Analysis | Reference-track A/B, export analysis report | known-signal LUFS/true-peak vectors |
| R12 | Customisation | Command palette, shortcut rebinding UI, light/dark pairs, toolbar customisation | command registry tests |

Order differs from the directive's list where dependencies force it: ids/undo before groups, markers and tempo map
before comping and launcher, and the workspace host before the editors that depend on a multi-panel layout.

## 11. Cross-cutting testing plan

Per milestone: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, wasm `cargo check`, Android debug APK build (all already in CI). Add: serialization
round trip of every new chunk; opening a new file in an old build must not corrupt it (fixture test);
complicated undo sequences across tracks, groups, routing, automation and mixer; `rt_alloc` extended to new engine
paths. Not covered by anything today and needs manual verification on devices: Android audio latency and
hot-plug, desktop device changes, plugin editors. Compatibility with other DAW formats is claimed only for what
`tests/flm_*` and FLP tests exercise; nothing else should be advertised.

## 12. Open decisions for the owner

1. Native format: extend `.flm` with Honeydew chunks (recommended) or a new format.
2. Workspace scope on phones: keep current one-editor layout below a width threshold (recommended).
3. First milestone to start: R0, then R1/R2 (small, unblock the rest).
