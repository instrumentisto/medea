#!/usr/bin/env bash

# Gets latest nightly version which presents rustfmt.
last_nightly_with_rustfmt=$(curl https://rust-lang.github.io/rustup-components-history/x86_64-unknown-linux-gnu/rustfmt)
RUSTFMT_RUST_VER=nightly-${last_nightly_with_rustfmt}
export RUSTFMT_RUST_VER

rustup toolchain install "${RUSTFMT_RUST_VER}"
rustup component add rustfmt --toolchain "${RUSTFMT_RUST_VER}"-x86_64-unknown-linux-gnu
