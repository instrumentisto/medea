#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
FROM rust:stretch AS dist

RUN cargo install wasm-pack \
    && rustup target add wasm32-unknown-unknown

WORKDIR /sources
COPY / .

RUN cd jason && wasm-pack build -t web


#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://hub.docker.com/_/nginx
FROM nginx:stable-alpine AS runtime

COPY jason/demo/chart/medea-demo/conf/nginx.vh.conf /etc/nginx/conf.d/default.conf

WORKDIR /app
COPY jason/demo/index.html .
COPY --from=dist /sources/jason/pkg/ ./js/

