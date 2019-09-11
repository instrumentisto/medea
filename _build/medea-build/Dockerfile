#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
ARG rust_ver=latest
FROM rust:${rust_ver} AS dist
ARG rustc_mode=release
ARG rustc_opts=--release
ARG cargo_home=/usr/local/cargo

COPY / /app/

# Create the user and group files that will be used in the running container to
# run the process as an unprivileged user.
RUN mkdir -p /out/etc/ \
 && echo 'nobody:x:65534:65534:nobody:/:' > /out/etc/passwd \
 && echo 'nobody:x:65534:' > /out/etc/group

RUN bash /app/_ci/protoc_install.sh

RUN apt-get update
RUN apt-get install -y cmake golang
ENV GOPATH /usr/lib/go

WORKDIR "/app"
