#
# Dockerfile of instrumentisto/medea-control-api-mock Docker image.
#


#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM ghcr.io/instrumentisto/rust:${rust_ver} AS dist
ARG rustc_mode=release
ARG rustc_opts=--release

# Create user and group files, which will be used in a running container to
# run the process as an unprivileged user.
RUN mkdir -p /out/etc/ \
 && echo 'nobody:x:65534:65534:nobody:/:' > /out/etc/passwd \
 && echo 'nobody:x:65534:' > /out/etc/group

# Install required system packages for building.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
            cmake

# Prepare Cargo workspace for building dependencies only.
COPY crates/medea-macro/Cargo.toml /app/crates/medea-macro/
COPY crates/medea-reactive/Cargo.toml /app/crates/medea-reactive/
COPY crates/medea-coturn-telnet-client/Cargo.toml \
     /app/crates/medea-coturn-telnet-client/
COPY mock/control-api/Cargo.toml /app/mock/control-api/
COPY proto/client-api/Cargo.toml /app/proto/client-api/
COPY proto/control-api/Cargo.toml /app/proto/control-api/
# Required to omit triggering re-compilation for build.rs.
COPY proto/control-api/build.rs /app/proto/control-api/
COPY proto/control-api/src/grpc/api.proto \
     proto/control-api/src/grpc/api*.rs \
     /app/proto/control-api/src/grpc/
COPY jason/Cargo.toml /app/jason/
COPY jason/jason-hello-world/Cargo.toml /app/jason/jason-hello-world/
COPY Cargo.toml Cargo.lock /app/
WORKDIR /app/
RUN mkdir -p crates/medea-macro/src/ && touch crates/medea-macro/src/lib.rs \
 && mkdir -p crates/medea-reactive/src/ \
 && touch crates/medea-reactive/src/lib.rs \
 && mkdir -p crates/medea-coturn-telnet-client/src/ \
 && touch crates/medea-coturn-telnet-client/src/lib.rs \
 && mkdir -p mock/control-api/src/ && touch mock/control-api/src/lib.rs \
 && mkdir -p proto/client-api/src/ && touch proto/client-api/src/lib.rs \
 && mkdir -p proto/control-api/src/ && touch proto/control-api/src/lib.rs \
 && mkdir -p jason/src/ && touch jason/src/lib.rs \
 && mkdir -p jason/jason-hello-world/src && touch jason/jason-hello-world/src/lib.rs \
 && mkdir -p src/ && touch src/lib.rs

# Build dependencies only.
RUN cargo build -p medea-control-api-mock ${rustc_opts}
# Remove fingreprints of pre-built empty project sub-crates
# to rebuild them correctly later.
RUN rm -rf /app/target/${rustc_mode}/.fingerprint/medea*

# Prepare project sources for building.
COPY mock/control-api /app/mock/control-api
COPY proto/control-api /app/proto/control-api

# Build project distribution binary.
# TODO: use --out-dir once stabilized
# TODO: https://github.com/rust-lang/cargo/issues/6790
RUN cargo build -p medea-control-api-mock ${rustc_opts}

# Prepare project distribution binary and all dependent dynamic libraries.
RUN cp /app/target/${rustc_mode}/medea-control-api-mock \
       /out/medea-control-api-mock \
 && ldd /out/medea-control-api-mock \
        # These libs are not reported by ldd(1) on binary,
        # but are vital for DNS resolution.
        # See: https://forums.aws.amazon.com/thread.jspa?threadID=291609
        /lib/x86_64-linux-gnu/libnss_dns.so.2 \
        /lib/x86_64-linux-gnu/libnss_files.so.2 \
    | awk 'BEGIN{ORS=" "}$1~/^\//{print $1}$3~/^\//{print $3}' \
    | sed 's/,$/\n/' \
    | tr -d ':' \
    | tr ' ' "\n" \
    | xargs -I '{}' cp -fL --parents '{}' /out/ \
 && rm -rf /out/out




#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://hub.docker.com/_/scratch
FROM scratch AS runtime

COPY --from=dist /out/ /

USER nobody:nobody

LABEL org.opencontainers.image.source="\
    https://github.com/instrumentisto/medea/tree/master/mock/control-api"

ENTRYPOINT ["/medea-control-api-mock"]
