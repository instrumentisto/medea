/// Compiles `trampoline.c` and links it into the final library.
fn main() {
    #[cfg(not(feature = "cargo-clippy"))]
    {
        println!("cargo:rerun-if-changed=src/include/trampoline.c");
        cc::Build::new()
            .file("src/dart_api_dl/trampoline.c")
            .compile("trampoline");
    }
}
