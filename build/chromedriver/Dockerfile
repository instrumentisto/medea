FROM alpine

# Installs latest Chromium package.
RUN echo @edge http://nl.alpinelinux.org/alpine/edge/community >> /etc/apk/repositories \
    && echo @edge http://nl.alpinelinux.org/alpine/edge/main >> /etc/apk/repositories \
    && apk add --no-cache \
    chromium@edge \
    harfbuzz@edge \
    nss@edge \
    freetype@edge \
    ttf-freefont@edge \
    # Install chromedriver
    chromium-chromedriver@edge \
    xvfb \
    # Cleanup cache
    && rm -rf /var/cache/* \
    && mkdir /var/cache/apk

ENV CHROME_BIN=/usr/bin/chromium-browser \
    CHROME_PATH=/usr/lib/chromium/ \
    DISPLAY=:20

RUN printf "Xvfb :20 -screen 0 1366x768x16 &\n chromedriver --whitelisted-ips=" > run.sh \
    && chmod +x run.sh

ENTRYPOINT ["/bin/sh", "run.sh"]
