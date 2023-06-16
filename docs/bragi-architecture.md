Bragi Architecture
==================

  * [Execution / Configuration](#execution-configuration)
  * [Common Primary Adapters](#common-primary-adapters)
  * [REST Adapter](#rest-adapter)
  * [Routing / Handling](#routing-handling)

Bragi is a web application providing a REST interface for querying Elasticsearch in the context of
Mimirsbrunn. By that I mean it can only be used to query data that have been previously been stored
in Elasticsearch by one of mimirsbrunn's binary.

Since Mimirsbrunn follows a hexagonal architecture, one part of bragi must be an adapter (aka
controller).  That is, one component of bragi must _adapt_ the input data from the http / REST
interface to the primary port.

So Bragi's code is divided in three sections:
1. The part of the code dedicated to its configuration, and its execution.
2. The part of the code common with other primary adapters
3. The part of the code specific to Bragi's primary adapter.

# <a id="execution-configuration"></a> Execution / Configuration

The part of the code dealing with command line arguments, configuration, and launching the web
server.

We find that code in `src/bragi`:

- [`src/bragi/main.rs`](/crates/bins/geocoder-server/src/main.rs) contains code for dealing with command-line, and
  delegate the subsequent execution to:
- [`src/bragi/server.rs`](/crates/bins/geocoder-server/src/server.rs) performs the following:
    1. initializes the logging / tracing,
    2. creates a configuration (see `src/settings.rs`)
    3. initializes a connection to the backend storage (Elasticsearch)
    4. creates the API object responsible for the server's functionality
    5. calls warp to serve the API.
- [`src/bragi/settings.rs`](/crates/bins/geocoder-server/src/settings.rs) merges the information from the command-line,
  from stored configuration files, and from environment variable to create a configuration.

You can find more about the organization of bragi's configuration in the [configuration
section](#configuration).

# <a id="common-primary-adapters"></a> Common

This is the code that will be available to all primary adapters.

Found in `libs/mimir/src/adapters/primary/common`:

- [`common/settings.rs`](/libs/mimir/src/adapters/primary/common/settings.rs) contains the query
  settings used to parameterize the query dsl sent by bragi to Elasticsearch. The settings are read
  from file configuration (or possibly sent by POST in debugging / test environments)

- [`common/filters.rs`](/libs/mimir/src/adapters/primary/common/filters.rs) contains a `struct
  Filter` which contains all the user supplied information to tweak the query dsl.

- [`common/dsl.rs`](/libs/mimir/src/adapters/primary/common/dsl.rs) contains the code to create a
  query dsl.

# REST Adapter

Found in `libs/mimir2/src/adapters/primary/bragi`:

- `bragi/routes`: each REST endpoint constitute a route

- `bragi/api`: All the structures used by the REST API to receive and transmit
  data, including for example response body, error

- `bragi/handlers`: ultimately a REST endpoint (or route) must make a call to
  the backend, and this is the role of the handler.

## Configuration

Bragi's configuration is split in three:
1. One section deal with the web server (in `config/bragi/`),
2. another section deal with the connection to Elasticsearch (in `config/elasticsearch/`),
2. and the section about tweaking the query dsl (in `config/query/`).

The reason for splitting the configuration is that you may need one and not the other: You don't
necessarily need to go through Bragi's web server to query Elasticsearch.

We use a layered approach to the configuration. First there is a default configuration, which is
always read (`default.toml`). Then, depending on the setting (ie dev, prod, test, ...) we override
with a corresponding configuration (`dev.toml`, `prod.toml`, ...). Finally we override with
environment variables and command line arguments.

# <a id="routing-handling"></a> Routing and Handling

How does a route works?

Let's look at the main autocomplete endpoint, which is often referred to by its more technical term,
forward geocoder (even if its not exactly a geocoder).

```rust
    let api = routes::forward_geocoder()
        .and(routes::with_client(client))
        .and(routes::with_settings(settings.query))
        .and_then(handlers::forward_geocoder)
        .recover(routes::report_invalid)
        .with(warp::trace::request());
```

Bragi uses the [warp](https://crates.io/crates/warp) web server framework, which combines *Filter*s
to achieve its functionality. 

So the first Filter is the route `forward_geocoder` which is

```
pub fn forward_geocoder() -> impl Filter<Extract = (InputQuery,), Error = Rejection> + Clone {
    warp::get()
        .and(path_prefix())
        .and(warp::path("autocomplete"))
        .and(forward_geocoder_query())
}
```

That is this filter will go through if the request is an HTTP GET, if the path is prefixed...

```rust
fn path_prefix() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    path!("api" / "v1" / ..).boxed()
}
```

and then followed by 'autocomplete', and finally if we can extract valid query parameters.

If this is the case, then we pass in to subsequent filters the backend (client), and a data
structure to construct the query DSL (`settings.query`). At that point, the next layer is handed 3
arguments:
* input query parameters from the first filter (`routes::forward_geocoder`)
* elasticsearch client connection (`routes::with_client`)
* query settings (`routes::with_settings`)

so the handler, which does the actual request to the primary port, builds a search response using
the geocode json format, and pass it to the next layer. We'll see later if things fall through

```rust
pub async fn forward_geocoder(
    params: InputQuery,
    client: ElasticsearchStorage,
    settings: settings::QuerySettings,
) -> Result<impl warp::Reply, warp::Rejection> {

    let q = params.q.clone();
    let filters = filters::Filters::from(params);
    let dsl = dsl::build_query(&q, filters, &["fr"], &settings);

    match client.search_documents([...], Query::QueryDSL(dsl)).await
    {
        Ok(res) => {
            let places: Result<Vec<Place>, serde_json::Error> = res
                .into_iter()
                .map(|json| serde_json::from_value::<Place>(json.into()))
                .collect();

            match places {
                Ok(places) => {
                    let features = places
                        .into_iter()
                        .map(|p| Feature::from_with_lang(p, None))
                        .collect();
                    let resp = GeocodeJsonResponse::new(q, features);
                    Ok(with_status(json(&resp), StatusCode::OK))
                }
                [...]
            }
        }
  [...]
    }
}
```

The last two filters of the autocomplete route are

```rust
.recover(routes::report_invalid)
.with(warp::trace::request());
```

and they ensure that any error happening in any of the preceding layer is correctly handled, and 
that we trace queries.
