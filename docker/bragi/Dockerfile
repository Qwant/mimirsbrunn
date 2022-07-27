# Builder image
# =============

FROM rust:1.60-alpine3.15 as builder

WORKDIR /home

RUN apk --update --no-cache add pkgconfig make sqlite-dev libressl-dev git musl-dev

RUN cargo new mimirsbrunn

WORKDIR /home/mimirsbrunn

COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
COPY ./libs ./libs
COPY ./config ./config
COPY ./docker ./docker
COPY ./build.rs ./build.rs
COPY ./tests ./tests
COPY ./benches ./benches

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/home/mimirsbrunn/target  \
    cargo build --profile production --bin bragi --locked --features db-storage

# Extract binary from build cache
RUN mkdir bin
RUN --mount=type=cache,target=/home/mimirsbrunn/target \
    cp target/production/bragi bin/


# Target image
# ============

FROM rust:1.60-alpine3.15

WORKDIR /srv

# curl required for healthchecks
RUN apk --update --no-cache add curl sqlite 

COPY config /etc/bragi
COPY docker/run_with_default_config.sh .
COPY --from=builder /home/mimirsbrunn/bin/bragi /usr/bin/bragi

EXPOSE 4000

ENV CONFIG_DIR  /etc/bragi
ENV RUST_LOG    info,hyper=info

ENTRYPOINT [ "./run_with_default_config.sh", "bragi" ]
