//! Compiles `trampoline.c` and links it into the final library.

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CLIPPY_ARGS");
    if env::var("CLIPPY_ARGS").is_ok() {
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
