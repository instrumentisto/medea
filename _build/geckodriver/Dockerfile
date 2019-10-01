FROM debian:stretch-slim

ARG firefox_ver=latest
ARG geckodriver_ver=v0.24.0

RUN apt-get update \
    && apt-get install -y \
    wget tar bzip2 \
    # Install dependencies for firefox binary
    `apt-cache depends firefox-esr | awk '/Depends:/{print$2}'` \
    # Install firefox
    && wget -O FirefoxSetup.tar.bz2 "https://download.mozilla.org/?product=firefox-${firefox_ver}&os=linux64&lang=en-US" \
    && tar xjf FirefoxSetup.tar.bz2 \
    && mv firefox /opt/firefox \
    # Install geckodriver
    && wget https://github.com/mozilla/geckodriver/releases/download/${geckodriver_ver}/geckodriver-${geckodriver_ver}-linux64.tar.gz \
    && tar -xvzf geckodriver* \
    && chmod +x geckodriver \
    && mv geckodriver /usr/bin

EXPOSE 4444

ENTRYPOINT ["geckodriver", "-b", "/opt/firefox/firefox", "--log", "debug"]
