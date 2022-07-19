Bragi
=====

  * [Getting Started](#getting-started)
  * [Usage](#usage)
    * [Configuration](#configuration)
  * [REST API](#rest-api)
    * [Forward Geocoding](#forward-geocoding)
    * [Reverse Geocoding](#reverse-geocoding)
    * [Status](#status)
    * [Features](#features)
    * [Explain Geocoding](#explain-geocoding)

Bragi is a web application providing a REST interface for querying a geospatial backend.
Bragi currently only works with Elasticsearch.

# Getting Started

Bragi is a part of Mimirsbrunn, and so it is built like any other rust project. Assuming
you have setup a rust environment,

TODO Note about minimal rust version here and checking the rust version.

```sh
git clone git@github.com:hove-io/mimirsbrunn.git
cd mimirsbrunn
cargo build --release
```

This will create an executable in `./target/release/bragi`

# Usage

Before using Bragi, you need to have an Elasticsearch backend available with data
indexed beforehand. Instructions on indexing are found [there](./indexing.md).

To start Bragi, you need to specify a configuration directory, and a run_mode.

For example, to start bragi for testing, and to look for the configuration in the
repository's config directory, run the following command.

```
./target/release/bragi -c ./config -m testing run
```

## Configuration

Bragi's configuration is split into three sections:
- parameters needed to tune the performance of the query.
- parameters to connect to the backend
- and the rest, focused bragi as a service (logging, port, ...)

The reason to split the configuration is that the part related to the query is
used in other contexts than Bragi.

The part related to the query is found in the `query` folder under the config base directory,
specified at the command line with `-c <config>`. The rest is found in `bragi`. So typically,
you will find:

```
bragi -c ./config

config
  ├── query
  │     └── default.toml
  │
  ├── elasticsearch
        ├── default.toml
  │     └── testing.toml
  │
  └── bragi
        ├── default.toml
        ├── testing.toml
        ├── prod.toml
        └── [...]
```

Bragi uses a layered approach to configuration. It will start by reading the `default.toml`
configuration, and override it with the file corresponding to the run mode. So, in the previous
example, if you run `bragi -c ./config -m testing run`, it will read `./config/bragi/default.toml`,
and override any value with those found in `./config/bragi/testing.toml`. You can still override
some settings with local values, using a `./config/bragi/local.toml`.

The Bragi configuration allows you to specify
* where the log files are stored,
* and on what address / port to serve Bragi

Here is an example:

```toml
[logging]
path = "./logs"

[service]
host = "0.0.0.0"
port = "6010"
```

Before running bragi, you may find it useful to see what bragi will use as a configuration. So there
is a `config` subcommand, which compiles the configuration, and prints it as a json object:

```
bragi -c ./config -m testing -s elasticsearch.port=9208 config
{
  "mode": "testing",
  "logging": {
    "path": "./logs"
  },
  "elasticsearch": {
    "host": "localhost",
    "port": 9201,
    "version_req": ">=7.13.0",
    "timeout": 100
  },
  "query": {
    "type_query": {
      "global": 30.0,
  […]
}
```

# REST API

Bragi exposes a small REST API summarized in the table below:

<!-- docs/assets/tbl/bragi-api.md -->

<table>
<colgroup>
<col style="width: 21%" />
<col style="width: 53%" />
<col style="width: 24%" />
</colgroup>
<thead>
<tr class="header">
<th>URL</th>
<th>Description</th>
<th>Details</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td><code>autocomplete</code></td>
<td><p>Searches the backend for places that match the query string.</p>
<p>Bragi acts as a forward geocoder.</p></td>
<td><a href="#forward-geocoding">link</a></td>
</tr>
<tr class="even">
<td><code>reverse</code></td>
<td><p>Searches the backend for places near the given location.</p>
<p>Bragi acts as a reverse geocoder.</p></td>
<td><a href="#reverse-geocoding">link</a></td>
</tr>
<tr class="odd">
<td><code>features</code></td>
<td>Returns Bragi’s status as well al the backend’s.</td>
<td><a href="#features">link</a></td>
</tr>
<tr class="even">
<td><code>status</code></td>
<td>Returns Bragi’s status as well al the backend’s.</td>
<td><a href="#status">link</a></td>
</tr>
<tr class="odd">
<td><code>autocomplete-explain</code></td>
<td>Return scoring details to analyze rankings</td>
<td><a href="#explain">link</a></td>
</tr>
</tbody>
</table>

## Forward Geocoding

Get a list of places (administrative regions, streets, ...) that best match your query string

**URL** : `/api/v1/autocomplete/`

**Method** : `GET`

### Query Parameters

TODO How to specify negative long lat ?

<!-- docs/assets/tbl/autocomplete-query-param.md -->

<table>
<colgroup>
<col style="width: 11%" />
<col style="width: 16%" />
<col style="width: 47%" />
<col style="width: 24%" />
</colgroup>
<thead>
<tr class="header">
<th>name</th>
<th>type</th>
<th>description</th>
<th>example</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>q</td>
<td>string</td>
<td>query string</td>
<td><code>q=lond</code></td>
</tr>
<tr class="even">
<td>lat</td>
<td>double (optional)</td>
<td>latitude. Used to boost results in the vicinity</td>
<td><code>lat=45.3456</code></td>
</tr>
<tr class="odd">
<td>lon</td>
<td>double (optional)</td>
<td>longitude. Note that if you specify lat or lon, you must specify the converse.</td>
<td><code>lon=2.4554</code></td>
</tr>
<tr class="even">
<td>datasets</td>
<td>list of strings (optional)</td>
<td><p>restrics the search to the given datasets.</p>
<p>Valid datasets values are specified at index time</p>
<p>See <a href="/docs/concepts.md">dataset</a> for an explanation of datasets.</p></td>
<td><code>datatasets[]=fr&amp;</code> <code>datasets[]=be</code></td>
</tr>
<tr class="odd">
<td>type</td>
<td>list of strings (optional)</td>
<td><p>restrics the search to the given place types.</p>
<p>Possible values are: * house, * poi, * public_transport:stop_area, * street, * zone</p>
<ol type="1">
<li>If no type is given, all types are searched.</li>
<li>This type parameter is featured in the response.</li>
<li>Some types require a <em>sub type</em>, eg poi =&gt; poi_type</li>
</ol></td>
<td><code>type[]=streets&amp;</code> <code>type[]=zone</code></td>
</tr>
<tr class="even">
<td>zone_type</td>
<td>list of strings (optional)</td>
<td>restrics the search to the given zone types. (1)</td>
<td><code>zone_type[]=city&amp;</code> <code>zone_type[]=city_district</code></td>
</tr>
<tr class="odd">
<td>shape_scope</td>
<td>list of strings</td>
<td>restrics the shape filter to the types listed in shape_scope.</td>
<td><code>shape_scope[]=street&amp;</code> <code>shape_scope[]=zone</code></td>
</tr>
</tbody>
</table>

TODO Finish

pub shape: Option<String>,
pub shape_scope: Option<Vec<String>>,
pub datasets: Option<Vec<String>>,
pub timeout: u32, // timeout to Elasticsearch in milliseconds

### Success Response

**Code** : `200 OK`

**Content examples**

The response is a JSON document which follows the
[geocodejson](https://github.com/geocoders/geocodejson-spec) specification.
Here is an example:

```json
{
  "type": "FeatureCollection",
  "geocoding": {
    "version": "0.1.0",
    "query": "hector"
  },
  "features": [
    {
      "type": "Feature",
      "geometry": {
        "coordinates": [
          2.3766059,
          48.8470632
        ],
        "type": "Point"
      },
      "properties": {
        "geocoding": {
          "id": "poi:osm:node:534918694",
          "type": "poi",
          "label": "Hector Malot (Paris)",
          "name": "Hector Malot",
          "postcode": "75012",
          "city": "Paris",
          "citycode": "75056",
          "administrative_regions": [
            {
              "id": "admin:osm:relation:2192616",
              "insee": "",
              "level": 10,
              "label": "Quartier des Quinze-Vingts (75012), Paris 12e Arrondissement, Paris, Île-de-France",
              "name": "Quartier des Quinze-Vingts",
  […]
}
```

### Failure Response

#### Bad Request

**Code** : `503 Internal Server Error`

**Content examples**

#### Internal Server Error

**Code** : `503 Internal Server Error`

**Content examples**

### Notes

## Reverse Geocoding

Reverse geocoding is an API endpoint to retrieve a list of places around geospatial coordinates.
This is to answer questions such as 'What are the public transportation stops around position x,y'.

Note that this functionality is used internally during the indexing step. For example, when we index
POIs, we try to enrich the raw input data by assigning an address. So we retrieve the POIs
coordinate, and ask the backend for the closest address.

### Query Parameters

<!-- docs/assets/tbl/reverse-query-param.md -->

<table>
<colgroup>
<col style="width: 7%" />
<col style="width: 15%" />
<col style="width: 58%" />
<col style="width: 17%" />
</colgroup>
<thead>
<tr class="header">
<th>name</th>
<th>type</th>
<th>description</th>
<th>example</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>lat</td>
<td>double</td>
<td></td>
<td><code>lat=45.3456</code></td>
</tr>
<tr class="even">
<td>lon</td>
<td>double</td>
<td></td>
<td><code>lon=2.4554</code></td>
</tr>
<tr class="odd">
<td>radius</td>
<td>string</td>
<td>Search radius, including a unit.</td>
<td><code>radius=50m</code></td>
</tr>
<tr class="even">
<td>type</td>
<td>list of strings (optional)</td>
<td><p>restrics the search to the given place types.</p>
<p>Possible values are: * house, * poi, * public_transport:stop_area, * street, * zone</p>
<ol type="1">
<li>If no type is given, only streets and addresses are searched.</li>
<li>This type parameter is featured in the response.</li>
</ol></td>
<td><code>type[]=streets&amp;</code> <code>type[]=zone</code></td>
</tr>
<tr class="odd">
<td>limit</td>
<td>integer</td>
<td>maximum number of places returned</td>
<td><code>limit=3</code></td>
</tr>
</tbody>
</table>

## Status

## Features

## Explain

## Testing

TODO

## Architecture

Bragi is a web application providing a REST interface for querying
Elasticsearch in the context of Mimirsbrunn. By that I mean it can only be used
to query data that have been previously stored in Elasticsearch by one of
mimirsbrunn's binary.

Since Mimirsbrunn follows a hexagonal architecture, one part of bragi must be
an adapter (aka controller).  That is, one component of bragi must _adapt_ the
input data from the http / REST interface to the primary port.

So Bragi's code is divided in three sections:
1. The part of the code dedicated to its configuration, and its execution.
2. The part of the code common with other primary adapters
3. The part of the code specific to Bragi's primary adapter.

### Execution

The part of the code dealing with command line arguments, configuration, and
launching the web server.

We find that code in `src/bragi`:

- `src/bragi/main.rs` contains code for dealing with command-line, and delegate
  the subsequent execution to:
- `src/bragi/server.rs` performs the following:
    1. initializes the logging / tracing,
    2. creates a configuration (see `src/settings.rs`)
    3. initializes a connection to the backend storage (Elasticsearch)
    4. creates the API object responsible for the server's functionality
    5. calls warp to serve the API.
- `src/bragi/settings.rs` merges the information from the command-line, from
  stored configuration files, and from environment variable to create a
  configuration.

### Common

This is the code that will be available to all primary adapters.

Found in `libs/mimir2/src/adapters/primary/common`:

- `common/settings.rs` contains the query settings used to parameterize the
  query dsl sent by bragi to Elasticsearch. The settings are read from file
  configuration (or possibly sent by POST in debugging / test environments)

- `common/filters.rs` contains a `struct Filter` which contains all the user
  supplied information to tweak the query dsl.

- `common/dsl.rs` contains the code to create a query dsl.

### REST Adapter

Found in `libs/mimir2/src/adapters/primary/bragi`:

- `bragi/routes`: each REST endpoint constitute a route

- `bragi/api`: All the structures used by the REST API to receive and transmit
  data, including for example response body, error

- `bragi/handlers`: ultimately a REST endpoint (or route) must make a call to
  the backend, and this is the role of the handler.


## Configuration

Bragi's configuration is split in two
1. One section deal with the web server and the connection to Elasticsearch,
2. The other is about the parameters that go into building a query for Elasticsearch.

The reason for splitting the configuration is that you may need one and not the
other: You don't necessarily need to go through Bragi's web server to query
Elasticsearch.

So the first part of the configuration is in `config/bragi`, while the other is
in `config/query`.

### Bragi's configuration

We use a layered approach to the configuration. First there is a default
configuration, which is always read (`default.toml`). Then, depending on the
setting (ie dev, prod, test, ...) we override with a corresponding
configuration (`dev.toml`, `prod.toml`, ...). Finally we override with
environment variables and command line arguments.

## Misc

How does a route works?

Let's look at the main autocomplete endpoint...

```rust
    let api = routes::forward_geocoder()
        .and(routes::with_client(client))
        .and(routes::with_settings(settings.query))
        .and_then(handlers::forward_geocoder)
        .recover(routes::report_invalid)
        .with(warp::trace::request());
```

Bragi uses the [warp](https://crates.io/crates/warp) web server framework,
which combines *Filter*s to achieve its functionality. 

So the first Filter is `forward_geocoder` which is

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

If this is the case, then we pass in to subsequent filters the backend (client), and a data structure to 
construct the query DSL (`settings.query`). At that point, the next layer is handed 3 arguments:
* input query parameters from the first filter (`routes::forward_geocoder`)
* elasticsearch client connection (`routes::with_client`)
* query settings (`routes::with_settings`)

so the handler, which does the actual request to the primary port, builds a
search response, and pass it to the next layer. We'll see later if things fall through

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
            let resp = SearchResponseBody::from(res);
            Ok(with_status(json(&resp), StatusCode::OK))
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
