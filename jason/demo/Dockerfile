# https://hub.docker.com/_/nginx
FROM nginx:stable-alpine


COPY conf/nginx.vh.conf /etc/nginx/conf.d/default.conf


COPY index.html /app/
COPY node_modules/jason-alexlapa/ /app/js/


WORKDIR /app
