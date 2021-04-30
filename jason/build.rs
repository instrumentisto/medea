//! Compiles `trampoline.c` and links it into the final library.

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CLIPPY_ARGS");
    if let Ok("cargo-clippy") = env::var("CARGO_CFG_FEATURE").as_deref() {
        return;
    }

    if let Ok("wasm32-unknown-unknown") = env::var("TARGET").as_deref() {
        return;
    }

    println!("cargo:rerun-if-changed=src/platform/dart/api_dl/trampoline.c");
    cc::Build::new()
        .file("src/platform/dart/api_dl/trampoline.c")
        .compile("trampoline");
}
