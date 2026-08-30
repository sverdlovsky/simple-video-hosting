FROM docker.io/library/rust:alpine AS build

RUN apk add \
    openssl-dev \
    musl-dev \
    perl \
    make \
    gcc

WORKDIR /srv/api

RUN cargo init

COPY Cargo.toml .
COPY Cargo.lock .

RUN cargo build --release

RUN cargo clean -rp svh

RUN rm -r src
COPY src src

RUN cargo build --release


FROM scratch

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /srv/api/target/release/svh /svh

USER 10001:10001

EXPOSE 8080

ENTRYPOINT [ "/svh" ]

