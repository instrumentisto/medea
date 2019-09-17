# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM rust:${rust_ver} AS dist

RUN apt-get update \
    && apt-get install -y cmake \
    && curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
