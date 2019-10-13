#
# Dockerfile of instrumentisto/medea Docker image.
#


#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM rust:${rust_ver} AS dist
ARG rustc_mode=release
ARG rustc_opts=--release

# Create user and group files, which will be used in a running container to
# run the process as an unprivileged user.
RUN mkdir -p /out/etc/ \
 && echo 'nobody:x:65534:65534:nobody:/:' > /out/etc/passwd \
 && echo 'nobody:x:65534:' > /out/etc/group

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
 cmake

COPY crates /app/crates/
COPY proto /app/proto/
COPY jason/Cargo.toml /app/jason/
COPY Cargo.toml Cargo.lock /app/

RUN cd /app \
 && mkdir -p src && touch src/lib.rs \
 && mkdir -p jason/src && touch jason/src/lib.rs \
 && cargo fetch

COPY src app/src

## Build project distribution.
RUN cd /app \
    # Compile project.
    # TODO: use --out-dir once stabilized
    # TODO: https://github.com/rust-lang/cargo/issues/6790
    && cargo build --bin=medea ${rustc_opts} \
 # Prepare the binary and all dependent dynamic libraries.
 && cp /app/target/${rustc_mode}/medea /out/medea \
 && ldd /out/medea \
    | awk 'BEGIN{ORS=" "}$1~/^\//{print $1}$3~/^\//{print $3}' \
    | sed 's/,$/\n/' \
    | tr ' ' "\n" \
    | xargs -I '{}' cp -fL --parents '{}' /out/




#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://hub.docker.com/_/scratch
FROM alpine AS runtime

COPY --from=dist /out/ /

USER nobody:nobody

ENTRYPOINT ["/medea"]
