fn main() {
    println!("cargo:rerun-if-changed=src/include/trampoline.c");
    cc::Build::new()
        .file("src/include/trampoline.c")
        .compile("trampoline");
}
