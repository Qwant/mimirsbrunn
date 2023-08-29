# Builder image
# =============

FROM rust:1.68-buster as builder

# libpostal is needed at both build and runtime
RUN apt-get update && apt-get install -y \
    autoconf automake build-essential curl git libsnappy-dev libtool pkg-config llvm-dev libclang-dev clang

WORKDIR /
RUN git clone https://github.com/openvenues/libpostal
WORKDIR /libpostal

COPY ./docker/tagger/build_libpostal.sh .
RUN ./build_libpostal.sh

WORKDIR /usr/src/tagger
COPY . .
RUN cargo build --release --package tagger-api --features address


FROM debian:buster-slim

RUN apt-get update && apt-get install -y \
    autoconf automake build-essential curl git libsnappy-dev libtool pkg-config llvm-dev libclang-dev clang

WORKDIR /
RUN git clone https://github.com/openvenues/libpostal
WORKDIR /libpostal

COPY ./docker/tagger/build_libpostal.sh .
RUN ./build_libpostal.sh

COPY --from=builder /usr/src/tagger/target/release/tagger-api /usr/bin/tagger
COPY ./docker/tagger/entrypoint.sh /entrypoint.sh

RUN useradd -ms /bin/sh tagger
USER tagger
WORKDIR /home/tagger
COPY ./crates/libs/tagger/assets/* .

EXPOSE 3000
ENV LD_LIBRARY_PATH=/lib:/usr/lib:/usr/local/lib
ENTRYPOINT [ "tagger" ]
CMD [ "--port", "8000", "--assets-path", "." ]
