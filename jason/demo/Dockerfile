# https://hub.docker.com/_/node/
FROM node:11-alpine AS deps

COPY / /npm/

RUN cd /npm && yarn


# https://hub.docker.com/_/nginx
FROM nginx:stable-alpine

COPY conf/nginx.vh.conf /etc/nginx/conf.d/default.conf
COPY index.html /app/
COPY --from=deps /npm/node_modules/jason-alexlapa/ /app/js/

WORKDIR /app
