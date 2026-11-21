fn main() {
    println!("cargo:rerun-if-changed=vendor");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("vendor/tape_adapter.cpp")
        .warnings(false)
        .compile("lemon_tape");
}
