ARG medea_build_image=instrumentisto/medea-build:latest
FROM ${medea_build_image} AS dist
ARG rustc_mode=release
ARG rustc_opts=--release
ARG cargo_home=/usr/local/cargo

# Create user and group files, which will be used in a running container to
# run the process as an unprivileged user.
RUN mkdir -p /out/etc/ \
 && echo 'nobody:x:65534:65534:nobody:/:' > /out/etc/passwd \
 && echo 'nobody:x:65534:' > /out/etc/group

COPY / /app/

# Build project distribution.
RUN cd /app \
 # Compile project.
 && CARGO_HOME="${cargo_home}" \
    # TODO: use --out-dir once stabilized
    # TODO: https://github.com/rust-lang/cargo/issues/6790
    cargo build -p control-api-mock ${rustc_opts} \
 # Prepare the binary and all dependent dynamic libraries.
 && cp /app/target/${rustc_mode}/control-api-mock /out/control-api-mock \
 && ldd /out/control-api-mock \
    | awk 'BEGIN{ORS=" "}$1~/^\//{print $1}$3~/^\//{print $3}' \
    | sed 's/,$/\n/' \
    | tr ' ' "\n" \
    | xargs -I '{}' cp -fL --parents '{}' /out/

FROM scratch AS runtime

COPY --from=dist /out/ /

USER nobody:nobody

ENTRYPOINT ["/control-api-mock"]