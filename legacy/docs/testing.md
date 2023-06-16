# Mimir Testing

## Testing and Docker

Some tests depend on the presence of an actual Elasticsearch, which can be easily obtained by installing the
corresponding docker container. Test execution should be idempotent. For example, if you run a test which creates an
index, and you try to run it twice without cleaning the index in between, you might fail the second time because an
index with the same name already exists. We have, at least known to me, two means of achieving this goal:

- `setup` and `teardown` functions
- using `Drop` trait

For the purpose of testing with an Elasticsearch docker container, these two solutions amount to:

- having a `setup` function which creates the container, and a `teardown` function which destroys the container.
- wrapping the container handle in something with a `Drop` implementation, so that when that something goes out of
    scope, we destroy the container.

Another aspect to take into account is the time we need to run the tests. If we create and destroy an Elasticsearch
container for every tests, the time it takes (about 5sec) for the Elasticsearch to be available is prohibitive. Tests
should be quick. 

To be (almost) sure that the tests would not interfere with a production environment, most tests use a function
`connection_test_pool()`, which requires the environment variable `ELASTICSEARCH_TEST_URL` to be set. Obviously we still
have the risk that the variable be set wrong...
