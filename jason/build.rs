/// Compiles `trampoline.c` and links it into the final library.
fn main() {
    println!("cargo:rerun-if-env-changed=CLIPPY_ARGS");
    if let Ok("cargo-clippy") = std::env::var("CARGO_CFG_FEATURE").as_deref() {
        return;
    }

    if true {
        return;
    }

    if let Ok("wasm32-unknown-unknown") = std::env::var("TARGET").as_deref() {
        return;
    }

    println!(
        "cargo:rerun-if-changed=src/platform/dart_ffi/dart_api_dl/trampoline.c"
    );
    cc::Build::new()
        .file("src/platform/dart_ffi/dart_api_dl/trampoline.c")
        .compile("trampoline");
}
