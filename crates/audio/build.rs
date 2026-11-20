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
    // Its 32-bit ARM has no vector divide, which Vital's own `NEON_ARM32` switch replaces by a reciprocal.
    if arch == "arm" {
        build.flag("-mfpu=neon").define("NEON_ARM32", None);
    }
    if arch == "x86" {
        build.flag("-msse2");
    }
    for file in ["engine.cpp", "load_save_subset.cpp", "bridge.cpp"] {
        build.file(root.join(file));
    }
    build.compile("vital_engine");
}

/// The JUCE modules Vital's interface is built from (each one's `include_juce_*.cpp`, in `vendor/vital-ui/juce/config`).
const JUCE_MODULES: &[&str] = &[
    "core",
    "events",
    "data_structures",
    "graphics",
    "gui_basics",
    "gui_extra",
    "opengl",
    "audio_basics",
    "audio_formats",
    "audio_processors",
    "audio_devices",
    "audio_utils",
    "dsp",
];

/// Vital's own translation units besides the engine's (`vendor/vital/src/unity_build/synthesis.cpp`).
const VITAL_UI_UNITS: &[&str] = &[
    "common",
    "interface_editor_components",
    "interface_editor_sections",
    "interface_editor_sections2",
    "interface_look_and_feel",
    "interface_wavetable",
];

/// The FreeType sources Vital's fonts need (JUCE draws text with it on Linux, which is what JUCE is built as here).
const FREETYPE: &[&str] = &[
    "base/ftbase.c",
    "base/ftbbox.c",
    "base/ftbitmap.c",
    "base/ftdebug.c",
    "base/ftfstype.c",
    "base/ftgasp.c",
    "base/ftglyph.c",
    "base/ftinit.c",
    "base/ftmm.c",
    "base/ftstroke.c",
    "base/ftsynth.c",
    "base/ftsystem.c",
    "cff/cff.c",
    "gzip/ftgzip.c",
    "psaux/psaux.c",
    "pshinter/pshinter.c",
    "psnames/psmodule.c",
    "raster/raster.c",
    "sfnt/sfnt.c",
    "smooth/smooth.c",
    "truetype/truetype.c",
];

/// Android: Vital's engine and its original interface. The engine is Vital's `SynthBase` (so the interface and the app
/// edit one synth), the interface is Vital's own `FullInterface` on JUCE, drawn windowless (EGL, OpenGL ES 3) into a
/// picture the app shows and sends touches to. `vendor/vital-ui/README.md` says how the pieces fit.
fn vital_ui() {
    let ui = Path::new("../../vendor/vital-ui");
    let vital = Path::new("../../vendor/vital");
    let freetype = Path::new("../../vendor/freetype");
    println!("cargo:rerun-if-changed={}", ui.display());
    println!("cargo:rerun-if-changed={}", vital.display());
    println!("cargo:rerun-if-changed={}", freetype.display());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // FreeType, in C.
    let mut ft = cc::Build::new();
    ft.opt_level(2)
        .debug(false)
        .warnings(false)
        .define("FT2_BUILD_LIBRARY", None)
        .define("FT_CONFIG_MODULES_H", Some("<ftmodule.h>"))
        .include(freetype)
        .include(freetype.join("include"));
    for file in FREETYPE {
        ft.file(freetype.join("src").join(file));
    }
    ft.compile("rind_freetype");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++14")
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .flag_if_supported("-fno-strict-aliasing")
        .flag_if_supported("-fexceptions")
        .flag_if_supported("-frtti")
        .flag_if_supported("-ffast-math")
        .flag_if_supported("-Wno-everything")
        .define("RIND_VITAL_UI", "1")
        .define("NO_AUTH", "1")
        .define("LINUX", "1")
        .define("NDEBUG", "1")
        .define("JUCE_HEADLESS_PLATFORM", "1")
        .define("OPENGL_ES", "1")
        .define("HEADLESS", "0")
        .define("JUCE_USE_XRANDR", "0")
        .define("JUCE_OPENGL3", "1")
        .define("JUCE_DSP_USE_SHARED_FFTW", "0")
        .define("JUCE_APP_VERSION", "1.0.6")
        .define("JUCE_APP_VERSION_HEX", "0x10006")
        .define("JUCE_USE_FREETYPE", "1");
    for define in [
        "Build_VST",
        "Build_VST3",
        "Build_AU",
        "Build_AUv3",
        "Build_RTAS",
        "Build_AAX",
        "Build_Standalone",
        "Build_Unity",
    ] {
        build.define(&format!("JucePlugin_{define}"), "0");
    }
    build
        .include(ui)
        .include(ui.join("juce/config"))
        .include(ui.join("juce/modules"))
        .include(ui.join("third_party"))
        .include(ui.join("third_party/concurrentqueue"))
        .include(vital.join("third_party"))
        .include(vital.join("third_party/json"))
        .include(vital.join("third_party/kissfft"))
        .include(freetype.join("include"));
    for dir in [
        "common",
        "common/wavetable",
        "interface/editor_components",
        "interface/editor_sections",
        "interface/look_and_feel",
        "interface/wavetable",
        "interface/wavetable/editors",
        "interface/wavetable/overlays",
    ] {
        build.include(ui.join("src").join(dir));
    }
    for dir in [
        "synth_engine",
        "effects",
        "filters",
        "framework",
        "lookups",
        "modulators",
        "modules",
        "producers",
        "utilities",
    ] {
        build.include(vital.join("src/synthesis").join(dir));
    }
    // Vital's SIMD (see `vital`).
    if arch == "arm" {
        build.flag("-mfpu=neon").define("NEON_ARM32", None);
    }
    for module in JUCE_MODULES {
        build.file(ui.join(format!("juce/config/include_juce_{module}.cpp")));
    }
    for unit in VITAL_UI_UNITS {
        build.file(ui.join(format!("unity/{unit}.cpp")));
    }
    build
        .file(vital.join("src/unity_build/synthesis.cpp"))
        .file(ui.join("juce/config/BinaryData.cpp"))
        .file(ui.join("rind_gui.cpp"))
        .file(vital.join("bridge.cpp"));
    build.compile("vital_ui");
    println!("cargo:rustc-link-lib=EGL");
    println!("cargo:rustc-link-lib=GLESv3");
    println!("cargo:rustc-link-lib=log");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        vital_ui();
    } else {
        vital();
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
