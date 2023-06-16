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

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./config ./config
COPY ./crates ./crates
COPY ./docker ./docker
COPY ./build.rs ./build.rs
COPY ./tests ./tests
COPY ./benches ./benches

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/home/mimirsbrunn/target

RUN cargo build --profile production --package geocoder-server  --locked

# Extract binary from build cache
RUN mkdir bin
RUN --mount=type=cache,target=/home/mimirsbrunn/target \
    cp target/production/geocoder-server bin/


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

COPY config /etc/bragi
COPY docker/run_with_default_config.sh .
COPY --from=builder /home/mimirsbrunn/bin/geocoder-server /usr/bin/bragi

EXPOSE 4000

ENV CONFIG_DIR  /etc/bragi
ENV RUST_LOG    info,hyper=info

ENTRYPOINT [ "./run_with_default_config.sh", "bragi" ]
