FROM debian:stretch-slim

RUN apt-get update && apt-get install -y wget tar bzip2 libgtk-3-0 libdbus-glib-1-2 libxt6

RUN wget -O FirefoxSetup.tar.bz2 "https://download.mozilla.org/?product=firefox-latest&os=linux64&lang=en-US"
RUN tar xjf FirefoxSetup.tar.bz2
RUN mv firefox /opt/firefox

RUN wget https://github.com/mozilla/geckodriver/releases/download/v0.24.0/geckodriver-v0.24.0-linux64.tar.gz
RUN tar -xvzf geckodriver*
RUN chmod +x geckodriver
RUN mv geckodriver /usr/bin

EXPOSE 4444

ENTRYPOINT ["geckodriver", "-b", "/opt/firefox/firefox"]
