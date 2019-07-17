#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/rust
FROM medea-build AS dist
ARG rustc_mode=release
ARG rustc_opts=--release
ARG cargo_home=/usr/local/cargo

# Create the user and group files that will be used in the running container to
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
    cargo build --bin=medea ${rustc_opts} \
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
FROM scratch AS runtime

COPY --from=dist /out/ /

USER nobody:nobody

ENTRYPOINT ["/medea"]
