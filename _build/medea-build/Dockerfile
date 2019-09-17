# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM rust:${rust_ver} AS dist

RUN apt-get update \
    # Dependencies for grpcio
    && apt-get install -y cmake
