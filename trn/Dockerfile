FROM docker.io/library/rust:alpine AS build

RUN apk add \
    openssl-dev \
    musl-dev \
    perl \
    make \
    gcc

WORKDIR /srv/trn

RUN cargo init

COPY Cargo.toml .
COPY Cargo.lock .

RUN cargo build --release

RUN cargo clean -rp svh

RUN rm -r src
COPY src src

RUN cargo build --release


FROM docker.io/library/alpine:latest

RUN apk add --no-cache ffmpeg

COPY --from=build /srv/trn/target/release/svh /usr/bin/svh

RUN addgroup -g 10001 appgroup && \
    adduser -D -u 10001 -G appgroup appuser

USER 10001:10001

ENTRYPOINT [ "/usr/bin/svh" ]

