FROM rust:1.33

COPY / /usr/src/medea
WORKDIR /usr/src/medea

RUN cargo install --path /usr/src/medea

CMD ["medea"]
