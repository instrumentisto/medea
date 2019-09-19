FROM medea-build:latest

COPY / /app

RUN cd /app && \
    cargo build -p control-api-mock

ENTRYPOINT ["cargo", "run", "-p", "control-api-mock", "--", "--addr", "127.0.0.1:8090"]