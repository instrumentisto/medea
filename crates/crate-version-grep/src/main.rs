use std::env;

use cargo_lock::lockfile::Lockfile;

fn main() {
    let crate_name = env::args()
        .skip(1)
        .next()
        .expect("Crate name not provided!");
    let lockfile = Lockfile::load("Cargo.lock").unwrap();

    for package in lockfile.packages {
        if package.name.as_str() == &crate_name {
            println!("{}", package.version);
            return;
        }
    }
    panic!("Crate not found in Cargo.lock!");
}
