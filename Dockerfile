FROM rust:bookworm AS build

COPY . /src
RUN cargo install --path /src --locked

FROM debian:bookworm

LABEL org.opencontainers.image.source=https://github.com/deps-rs/deps.rs

RUN set -ex; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends ca-certificates libssl-dev libcurl4-openssl-dev; \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/cargo/bin/shiny-robots /usr/local/bin

RUN useradd -ms /bin/bash -u 1001 deps
WORKDIR /home/deps
USER deps

EXPOSE 8080
CMD ["/usr/local/bin/shiny-robots"]
