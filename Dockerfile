# syntax=docker/dockerfile:1

FROM rust:slim-bookworm AS build

COPY . /src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    CARGO_TARGET_DIR=/src/target cargo install --path /src --locked

FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/deps-rs/deps.rs

RUN set -ex; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/cargo/bin/shiny-robots /usr/local/bin

RUN useradd -ms /bin/bash -u 1001 deps
WORKDIR /home/deps
USER deps

EXPOSE 8080
CMD ["/usr/local/bin/shiny-robots"]
