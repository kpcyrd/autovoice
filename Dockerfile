# syntax=docker/dockerfile:1.4

FROM rust:1-alpine3.17 as build
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk add musl-dev
WORKDIR /app
COPY ./ /app
RUN --mount=type=cache,target=/var/cache/buildkit \
    CARGO_HOME=/var/cache/buildkit/cargo \
    CARGO_TARGET_DIR=/var/cache/buildkit/target \
    cargo build --release --locked && \
    cp -v /var/cache/buildkit/target/release/autovoice .
RUN strip autovoice

FROM alpine:3.17
# install dependencies
RUN apk add libgcc
# copy the binary
COPY --from=0 /app/autovoice /usr/bin
ENTRYPOINT ["autovoice"]
