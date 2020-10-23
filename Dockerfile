FROM buildpack-deps:buster as build

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

COPY rust-toolchain /src/

RUN set -eux; \
    \
    url="https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init"; \
    wget "$url"; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --default-toolchain $(cat /src/rust-toolchain); \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

COPY . /src
RUN cargo install --path /src

FROM debian:buster

LABEL org.opencontainers.image.source https://github.com/deps-rs/deps.rs

RUN set -ex; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends libssl-dev; \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/cargo/bin/shiny-robots /usr/local/bin

EXPOSE 8080
CMD /usr/local/bin/shiny-robots
