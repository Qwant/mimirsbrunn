Introduction to Mimirsbrunn
===========================

  * [Requirements](#requirements)
    * [Rust](#rust)
    * [Docker](#docker)
    * [Tools](#tools)
    * [Hardware](#hardware)
  * [Installing](#installing)

Mimirsbrunn is a geocoder, and contains both the tools to ingest data into a backend (Elasticsearch)
and a REST API to query the indexes thus created.

In this tutorial we're going to see most of the capabilities of Mimirsbrunn, focusing on the Ile de
France region (around Paris). 

# Requirements

## Rust

For this tutorial we assume the user has already a rust environment. See the [rust
website](https://www.rust-lang.org/tools/install) for details on installing rust on your platform of
choice. 

## Docker

For the purpose of this tutorial, running Elasticsearch in a docker environment is an adequate
solution. So we assume the user has a docker engine installed. See the [docker
website](https://docs.docker.com/engine/install/) for details on installing docker on your platform
of choice.

## Tools

You will also need:
* **git** to retrieve the source code,
* **curl** to make calls to Elasticsearch or the REST API,
* **jq** is optional, but very convenient to manipulate JSON.

## Hardware 

TODO Talk about memory for Elasticsearch.

# Installing

You need to retrieve the project and build it using the rust compiler:

```
git clone https://github.com/hove-io/mimirsbrunn.git
cd mimirsbrunn
cargo build --release
```

This will download all the project dependencies (crates in rust), and build static binaries in
the folder `./target/release`. 

