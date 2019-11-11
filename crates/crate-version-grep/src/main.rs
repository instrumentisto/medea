//! CLI application for getting current locked version of crate from
//! `Cargo.lock` file.
//!
//! # Usage
//!
//! ```bash
//! $ crate-version-grep {{ CRATE NAME }}
//! ```
//!
//! ## Example
//!
//! ```bash
//! $ crate-version-grep wasm-bingen # -> 0.2.53
//! ```

use std::env;

use cargo_lock::lockfile::Lockfile;

fn main() {
    let crate_name = env::args().nth(1).expect("Crate name not provided!");
    let lockfile = Lockfile::load("Cargo.lock").unwrap();

    for package in lockfile.packages {
        if package.name.as_str() == crate_name {
            println!("{}", package.version);
            return;
        }
    }
    panic!("Crate not found in Cargo.lock!");
}
