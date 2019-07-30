FROM debian:stretch-slim

ARG firefox_ver=latest
ARG geckodriver_ver=v0.24.0

RUN apt-get update && apt-get install -y wget tar bzip2 libgtk-3-0 libdbus-glib-1-2 libxt6

RUN wget -O FirefoxSetup.tar.bz2 "https://download.mozilla.org/?product=firefox-${firefox_ver}&os=linux64&lang=en-US"
RUN tar xjf FirefoxSetup.tar.bz2
RUN mv firefox /opt/firefox

RUN wget https://github.com/mozilla/geckodriver/releases/download/${geckodriver_ver}/geckodriver-${geckodriver_ver}-linux64.tar.gz
RUN tar -xvzf geckodriver*
RUN chmod +x geckodriver
RUN mv geckodriver /usr/bin

EXPOSE 4444

ENTRYPOINT ["geckodriver", "-b", "/opt/firefox/firefox"]
