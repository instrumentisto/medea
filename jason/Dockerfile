#
# Dockerfile of instrumentisto/medea-demo:edge Docker image.
#


#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM ghcr.io/instrumentisto/rust:${rust_ver} AS dist
ARG debug=no

RUN cargo install wasm-pack \
 && rustup target add wasm32-unknown-unknown

COPY / /src/

RUN cd /src/ \
 && make cargo.build crate=medea-jason debug=${debug} dockerized=no




#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://hub.docker.com/_/nginx
FROM nginx:stable-alpine AS runtime

COPY jason/demo/chart/medea-demo/conf/nginx.vh.conf \
     /etc/nginx/conf.d/default.conf

COPY jason/demo/index.html /app/
COPY --from=dist /src/jason/pkg/ /app/js/

WORKDIR /app

LABEL org.opencontainers.image.source="\
    https://github.com/instrumentisto/medea/tree/master/jason/demo"
