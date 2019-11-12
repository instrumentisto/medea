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
    Lockfile::load("Cargo.lock").unwrap().packages.into_iter()
        .skip_while(|package| package.name.as_str() != crate_name)
        .next()
        .map(|package| println!("{}", package.version))
        .or_else(|| panic!("Crate not found in Cargo.lock!"));
}
