// SPDX-License-Identifier: GPL-3.0-or-later
// The msfa engine and its glue, compiled with the C++ compiler of the target (the NDK's on Android).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let files = [
        "dx7note", "env", "exp2", "fm_core", "fm_op_kernel", "freqlut", "lfo", "patch", "pitchenv",
        "rdx", "sin",
    ];
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++14")
        // An FM engine at -O0 is too slow for a debug build: its DSP is always optimised.
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .include("cpp");
    for name in files {
        build.file(format!("cpp/{name}.cc"));
    }
    build.compile("rind_fm_engine");
}
