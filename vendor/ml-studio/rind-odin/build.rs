// SPDX-License-Identifier: GPL-3.0-or-later
// Odin 2's synth engine (C++20) and Honeydew's glue, built against a small stand-in for the parts of JUCE it uses
// (`cpp/shim`), compiled with the C++ compiler of the target (the NDK's on Android).
use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            sources(&p, out);
        } else if matches!(p.extension().and_then(|e| e.to_str()), Some("cpp" | "cc"))
            // The processor's parts are included by `OdinCore.cpp`, one translation unit.
            && !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("PluginProcessor"))
        {
            out.push(p);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut files = Vec::new();
    sources(Path::new("cpp/Source"), &mut files);
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        // A polysynth at -O0 is too slow for a debug build: its DSP is always optimised.
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .define("NDEBUG", None)
        // Rind OB's engine (OB-Xd) has classes of the same names: one program, one definition each, or the linker picks
        // one at random and a synth runs the other's code.
        .define("Voice", "OdinVoice")
        .define("ADSREnvelope", "OdinADSREnvelope")
        .include("cpp/Source")
        .include("cpp/shim")
        .include("cpp")
        .files(files);
    build.compile("rind_odin_engine");
}
