#
# Dockerfile of instrumentisto/medea-demo:latest Docker image.
#


#
# Stage 'dist' creates project distribution.
#

# https://hub.docker.com/_/node
FROM node:alpine AS dist

COPY / /npm/

RUN cd /npm/ \
 && yarn install --pure-lockfile




#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://hub.docker.com/_/nginx
FROM nginx:stable-alpine AS runtime

COPY chart/medea-demo/conf/nginx.vh.conf /etc/nginx/conf.d/default.conf

COPY index.html /app/
COPY --from=dist /npm/node_modules/medea-jason/ /app/js/

WORKDIR /app

LABEL org.opencontainers.image.source="\
    https://github.com/instrumentisto/medea/tree/master/jason/demo"
