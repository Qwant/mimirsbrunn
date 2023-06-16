# Testing

// Copied from front page, to improve

Since this is a rust project, we are well instrumented to run all sorts of tests:
* style
* lint
* unit tests
* end to end / integration.
* benchmark

You can run them all at once, and this in the way it is carried out in the CI pipeline, with 

```
make check
```

See this [page](/docs/testing.md) for a more in depth introduction to testing this project.

### Style

We don't follow any particular style, just the default rust style, and you can check that your
contribution follows the style with:

```
cargo fmt --all -- --check
```

### Lint

Again we follow the linting suggestions given by *clippy*:

```
cargo clippy --all-targets -- --warn clippy::cargo --allow clippy::multiple_crate_versions --deny warnings
```

#### Unit Tests

TODO

#### End to End Tests

TODO

More information about the project can be found in [the docs subdirectory](docs/README.md).

The project has all kinds of testing: unit tests, end to end tests, and benchmarks. Some of these
tests require test doubles (we use mocking), while others require an elasticsearch.

## Docker and Elasticsearch.

Some tests need Elasticsearch. We find it most convenient to use a docker container for that purpose.
These tests start with:

```rust
#[cfg(test)]
pub mod tests {
  use mimir2::utils::docker;
  #[tokio:test]
  #[serial]
  async fn should_return_something_blue() {
  
    test_containers::initialize().await.expect("docker initialization of unit test");
  
    // do something with elasticsearch, like create indices, ...

  }
}
```

What's happening behind that call are a series of steps to ensure a clean Elasticsearch is available
as a docker container. For one, the test is prefixed with a directive, `#[serial]`. Indeed, if you
are using the same elasticsearch, and run several tests in parallel, you will run into concurrency
issues. With `#[serial]`, we can ensure that during that test, we have single read write access to
that container. It is at the expense of the speed of execution of those tests.

The `test_containers::initialize` checks to see if a docker container with the name of the default test
container is available. If none, it will create a container. The network configuration of the
container is drawn from the configuration found in `config/elasticsearch/testing.toml`. When the
container is started, we wait a bit for the container to be available. If a container is available
when we check, then we wipe it clean, that is we delete all its indices. At that point, we try to
initialize a client connection to elasticsearch, and if it is succesful, the docker is initialized.

Obviously there is a risk of wiping out a valid elasticsearch while running unit tests.

## Mocking and Hexagonal Architecture.

One of the motivation to use the hexagonal architecture for Mimirsbrunn was to improve the
testability of the project. The reason for that is that, while you can still run end to end tests
with an Elasticsearch as a backend, you can also run more narrow-focused test using the primary and
secondary domains as interface boundaries.

## Integration tests and Behavior Driven Development.

## Static and Dynamic Fixtures.

Sometimes to run meaningful tests, you need to have significantly large datasets. These cannot be
stored in the source tree. So large datasets are downloaded.
