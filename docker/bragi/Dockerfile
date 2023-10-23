FROM rust:1.71-buster as builder

ENV DEBIAN_FRONTEND noninteractive

RUN apt-get update \
 && apt-get install --assume-yes --quiet --no-install-recommends \
      libsqlite3-dev \
      libssl-dev \
      git \
      make \
      pkg-config \
 && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN USER=root

WORKDIR /src/mimir

COPY . .

RUN cargo build --release --package geocoder-server --locked
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/mimir/target

RUN cargo build --release --package geocoder-server  --locked

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
COPY --from=builder /src/mimir/target/release/geocoder-server /usr/bin/bragi

EXPOSE 4000

ENTRYPOINT [ "./entrypoint.sh", "bragi" ]
