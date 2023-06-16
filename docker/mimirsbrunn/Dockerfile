# Builder image
# =============

FROM rust:1.68-buster as builder

WORKDIR /home

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update
RUN apt-get install -y pkg-config make libsqlite3-dev libssl-dev git
RUN apt-get clean
RUN rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*;

RUN USER=root cargo new mimirsbrunn

WORKDIR /home/mimirsbrunn

COPY ./docker ./docker
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./build.rs ./build.rs
COPY ./config ./config
COPY ./benches ./benches
COPY ./tests ./tests
COPY ./crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/home/mimirsbrunn/target  \
    cargo build --profile production --workspace --locked --features db-storage

# Extract binaries from build cache
RUN mkdir bin
RUN --mount=type=cache,target=/home/mimirsbrunn/target      \
    cp target/production/osm-importer                       \
       target/production/cosmogony-importer                 \
       target/production/bano-importer                      \
       target/production/openaddresses-importer             \
       target/production/ctl-importer                      \
       bin/


# Target image
# ============

FROM debian:buster-slim

WORKDIR /srv

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update
# curl required for healthchecks
RUN apt-get install -y curl libcurl4 sqlite3
RUN apt-get clean
RUN rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY config /etc/mimirsbrunn
COPY docker/run_with_default_config.sh .
COPY --from=builder /home/mimirsbrunn/bin/osm-importer /usr/bin/osm2mimir
COPY --from=builder /home/mimirsbrunn/bin/cosmogony-importer /usr/bin/cosmogony2mimir
COPY --from=builder /home/mimirsbrunn/bin/bano-importer /usr/bin/bano2mimir
COPY --from=builder /home/mimirsbrunn/bin/openaddresses-importer /usr/bin/openaddresses2mimir
COPY --from=builder /home/mimirsbrunn/bin/ctl-importer /usr/bin/ctlmimir

ENTRYPOINT [ "./run_with_default_config.sh" ]
