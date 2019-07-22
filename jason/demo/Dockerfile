FROM nginx:alpine

COPY index.html /usr/share/nginx/html
COPY ./node_modules/jason-alexlapa/ /usr/share/nginx/html/js

ADD ./conf/mime.types /etc/nginx/mime.types