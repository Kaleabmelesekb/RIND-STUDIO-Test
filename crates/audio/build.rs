//! Builds the Dragonfly reverbs' original C++ DSP (`vendor/dragonfly-reverb`, see its README): Freeverb3 once, and each
//! plugin's own `DSP.cpp` with `bridge.cpp`, the C face the engine calls. The plugins share class names, so each is
//! built with its own name for `DragonflyReverbDSP`. The web build has no C++ toolchain target and leaves them out
//! (`dragonfly_fx` passes audio through there).

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

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
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
