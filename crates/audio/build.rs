//! Builds the Dragonfly reverbs' original C++ DSP (`vendor/dragonfly-reverb`, see its README): Freeverb3 once, and each
//! plugin's own `DSP.cpp` with `bridge.cpp`, the C face the engine calls. The plugins share class names, so each is
//! built with its own name for `DragonflyReverbDSP`. The web build has no C++ toolchain target and leaves them out
//! (`dragonfly_fx` passes audio through there).
//!
//! Also Vital's engine (`vendor/vital`): its own sources in one unity translation unit (`engine.cpp`, the order Vital's
//! `synthesis.cpp` needs), the small JUCE stand-in headers it was written against, and `bridge.cpp`, the C face the
//! `vital` module calls.

use std::path::Path;

/// The Freeverb3 sources the Dragonfly DSPs use.
const FREEVERB: &[&str] = &[
    "allpass",
    "biquad",
    "comb",
    "delay",
    "delayline",
    "earlyref",
    "efilter",
    "nrev",
    "nrevb",
    "progenitor",
    "progenitor2",
    "revbase",
    "slot",
    "strev",
    "utils",
    "zrev",
    "zrev2",
];

/// Each plugin: its entry-point prefix, its DSP class's new name and its folder.
const PLUGINS: &[(&str, &str, &str)] = &[
    ("df_hall", "DragonflyHallDSP", "dragonfly-hall-reverb"),
    ("df_room", "DragonflyRoomDSP", "dragonfly-room-reverb"),
    ("df_plate", "DragonflyPlateDSP", "dragonfly-plate-reverb"),
    (
        "df_early",
        "DragonflyEarlyDSP",
        "dragonfly-early-reflections",
    ),
];

/// Vital's engine: one library from its unity file, the three small files of this build and Vital's include roots.
fn vital() {
    let root = Path::new("../../vendor/vital");
    println!("cargo:rerun-if-changed={}", root.display());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        // Vital's DSP at -O0 cannot keep up in a debug build: it is always optimised.
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .flag_if_supported("-fno-strict-aliasing")
        // The shim comes first: its `JuceHeader.h` and `load_save.h` stand in for the JUCE ones.
        .include(root.join("shim"))
        .include(root)
        .include(root.join("third_party"));
    for dir in [
        "src/unity_build",
        "src/common",
        "src/common/wavetable",
        "src/synthesis/framework",
        "src/synthesis/lookups",
        "src/synthesis/utilities",
        "src/synthesis/filters",
        "src/synthesis/effects",
        "src/synthesis/modulators",
        "src/synthesis/modules",
        "src/synthesis/producers",
        "src/synthesis/synth_engine",
        "src/interface/look_and_feel",
    ] {
        build.include(root.join(dir));
    }
    // Vital's SIMD: SSE2 on x86 (the default of 64-bit), NEON on ARM, which 32-bit ARM must be asked for.
    if arch == "arm" {
        build.flag("-mfpu=neon");
    }
    if arch == "x86" {
        build.flag("-msse2");
    }
    for file in ["engine.cpp", "load_save_subset.cpp", "bridge.cpp"] {
        build.file(root.join(file));
    }
    build.compile("vital_engine");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    vital();
    let root = Path::new("../../vendor/dragonfly-reverb");
    println!("cargo:rerun-if-changed={}", root.display());
    let common = root.join("common");
    let shim = root.join("shim");

    let base = || {
        let mut build = cc::Build::new();
        build
            .cpp(true)
            .std("c++14")
            // A reverb at -O0 is too slow for a debug build: its DSP is always optimised.
            .opt_level(2)
            .debug(false)
            .warnings(false)
            .define("LIBFV3_FLOAT", None)
            .include(&common)
            .include(&shim);
        build
    };

    for (prefix, class, folder) in PLUGINS {
        let dir = root.join("plugins").join(folder);
        let mut plugin = base();
        plugin
            .include(&dir)
            .define("DF_PREFIX", Some(*prefix))
            .define("DragonflyReverbDSP", Some(*class))
            .file(dir.join("DSP.cpp"))
            .file(root.join("bridge.cpp"))
            .compile(&format!("{prefix}_dsp"));
    }

    // Last: a static library is searched only for what the ones before it left undefined.
    let mut freeverb = base();
    freeverb.include(common.join("freeverb"));
    for name in FREEVERB {
        freeverb.file(common.join("freeverb").join(format!("{name}.cpp")));
    }
    freeverb.compile("dragonfly_freeverb3");
}
