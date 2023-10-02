# Builder image
# =============

FROM rust:1.71-buster as builder

WORKDIR /home

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update \
 && apt-get install --assume-yes --quiet --no-install-recommends \
      git \
      make \
      libsqlite3-dev \
      libssl-dev \
      pkg-config \
 && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN USER=root cargo new mimirsbrunn

WORKDIR /home/mimirsbrunn

COPY ./docker ./docker
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./config ./config
COPY ./tests ./tests
COPY ./crates ./crates

RUN cargo build --workspace --release --locked --features db-storage


# Target image
# ============

FROM debian:buster-slim

WORKDIR /srv

ENV DEBIAN_FRONTEND noninteractive

# curl required for healthchecks
RUN apt-get update \
 && apt-get install --assume-yes --quiet --no-install-recommends \
      curl \
      libcurl4 \
      sqlite3 \
 && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY config /etc/mimirsbrunn
COPY docker/run_with_default_config.sh .
COPY --from=builder /home/mimirsbrunn/target/release/osm-importer /usr/bin/osm2mimir
COPY --from=builder /home/mimirsbrunn/target/release/cosmogony-importer /usr/bin/cosmogony2mimir
COPY --from=builder /home/mimirsbrunn/target/release/openaddresses-importer /usr/bin/openaddresses2mimir
COPY --from=builder /home/mimirsbrunn/target/release/ctl-importer /usr/bin/ctlmimir

ENTRYPOINT [ "./run_with_default_config.sh" ]
