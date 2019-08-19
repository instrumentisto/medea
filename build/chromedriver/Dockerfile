FROM debian:stretch-slim

ENV CHROMEDRIVER_VERSION "76.0.3809.68"

RUN echo "deb [arch=amd64]  http://dl.google.com/linux/chrome/deb/ stable main" >> /etc/apt/sources.list.d/google-chrome.list
RUN apt-get -y update
RUN apt-get install -y curl unzip xvfb libxi6 libgconf-2-4 gnupg2

RUN curl -sS -o - https://dl-ssl.google.com/linux/linux_signing_key.pub | apt-key add -
RUN apt-get -y --allow-unauthenticated install chromium

RUN mkdir -p /opt/chromedriver-$CHROMEDRIVER_VERSION \
  && curl -sS -o chromedriver_linux64.zip https://chromedriver.storage.googleapis.com/$CHROMEDRIVER_VERSION/chromedriver_linux64.zip \
  && unzip -qq chromedriver_linux64.zip -d /opt/chromedriver-$CHROMEDRIVER_VERSION \
  && rm chromedriver_linux64.zip \
  && chmod +x /opt/chromedriver-$CHROMEDRIVER_VERSION/chromedriver \
  && ln -fs /opt/chromedriver-$CHROMEDRIVER_VERSION/chromedriver /usr/local/bin/chromedriver

RUN chromium --version

ENV DISPLAY :20

RUN echo "Xvfb :20 -screen 0 1366x768x16 &\nchromedriver --whitelisted-ips=" > run.sh
RUN chmod +x run.sh

ENTRYPOINT ["/bin/bash", "run.sh"]

#ENTRYPOINT ["chromedriver", "--log-level=ALL", "--whitelisted-ips="]