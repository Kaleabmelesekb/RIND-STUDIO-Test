// SPDX-License-Identifier: GPL-3.0-or-later
// The Airwindows effects (cpp/fx, MIT) and Honeydew's glue, compiled with the C++ compiler of the target (the NDK's on
// Android).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        // Plain DSP loops at -O0 are too slow for a debug build: they are always optimised.
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .define("NDEBUG", None)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .include("cpp")
        .file("cpp/aw.cc")
        .file("cpp/airwin_consolidated_base.cpp");
    let files = std::fs::read_dir("cpp/fx").expect("cpp/fx");
    let mut files: Vec<_> = files
        .filter_map(|f| f.ok().map(|f| f.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "cpp"))
        .collect();
    files.sort();
    for f in files {
        build.file(f);
    }
    build.compile("rind_airwindows_engine");
}
