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

WORKDIR /home/mimirsbrunn

COPY . .

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
COPY docker/entrypoint.sh .
COPY --from=builder /home/mimirsbrunn/target/release/osm-importer /usr/bin/
COPY --from=builder /home/mimirsbrunn/target/release/cosmogony-importer /usr/bin/
COPY --from=builder /home/mimirsbrunn/target/release/openaddresses-importer /usr/bin/
COPY --from=builder /home/mimirsbrunn/target/release/ctl-importer /usr/bin/

ENTRYPOINT [ "./entrypoint.sh" ]
