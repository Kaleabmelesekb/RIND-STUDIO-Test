// SPDX-License-Identifier: GPL-3.0-or-later
// OB-Xf's synth engine (header-only, C++20, SIMD through simde) and Honeydew's glue, compiled with the C++ compiler of
// the target (the NDK's on Android).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        // A polysynth at -O0 is too slow for a debug build: its DSP is always optimised.
        .opt_level(2)
        .debug(false)
        .warnings(false)
        .define("NDEBUG", None)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .include("cpp")
        .include("cpp/core")
        .include("cpp/shim")
        .file("cpp/obx.cc");
    build.compile("rind_ob_engine");
}
