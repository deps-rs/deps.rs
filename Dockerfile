FROM rust:latest as build

COPY . /src
RUN cargo install --path /src

FROM debian:buster

LABEL org.opencontainers.image.source https://github.com/deps-rs/deps.rs

RUN set -ex; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends ca-certificates libssl-dev; \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/cargo/bin/shiny-robots /usr/local/bin

EXPOSE 8080
CMD /usr/local/bin/shiny-robots
