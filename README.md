[![travis](https://travis-ci.org/CanalTP/mimirsbrunn.svg?branch=master)](https://travis-ci.org/CanalTP/mimirsbrunn)
[![GitHub license](https://img.shields.io/github/license/CanalTP/mimirsbrunn.svg)](https://github.com/CanalTP/mimirsbrunn/blob/master/LICENSE)
[![GitHub tag](https://img.shields.io/github/tag/CanalTP/mimirsbrunn.svg)](https://github.com/CanalTP/mimirsbrunn/tag)
[![Quality Gate Status](https://sonarcloud.io/api/project_badges/measure?project=CanalTP_mimirsbrunn&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=CanalTP_mimirsbrunn)

Mimirsbrunn
=================

  * [What's a Geocoder](#whats-a-geocoder)
  * [Getting Started](#getting-started)
    * [Prerequisites](#prerequisites)
    * [Installing](#installing)
    * [Running](#running)
    * [Testing](#testing)
    * [Where to Go From There](#where-to-go-from-there)
  * [Development](#development)
    * [Contributing](#contributing)
    * [Versioning](#versioning)
    * [Authors](#authors)
  * [Processes](#processes)
  * [Who Uses It ?](#who-uses-it)
  * [Alternatives](#alternatives)
  * [Resources](#resources)
  * [License](#license)
  * [Acknowledgments](#acknowledgments)


Mimirsbrunn (also called Mimir) is an independent geocoding and reverse-geocoding system written in
[Rust](https://www.rust-lang.org/en-US/), and built upon [Elasticsearch](https://www.elastic.co). It
can handle addresses, streets, points-of-interest (POI), administrative regions and public transport
stops.

# What's a Geocoder ?

Usually [geocoding](https://en.wikipedia.org/wiki/Geocoding) refers to "the process of transforming
a physical address description to a location on the Earth's surface". However Mimir is more a
[geoparser](https://en.wikipedia.org/wiki/Toponym_resolution#Geoparsing) than a geocoder since it
can resolve any ambiguous toponym to its correct location.

In other words, a geocoder reads a description (possibly incomplete) of a location, and returns a
list of candidate locations (latitude / longitude) matching the input. 

Geocoding is traditionally used for autocompleting search fields used in geographic applications.
For example, here is a screenshot of Qwant Maps, where the user enters a search string `20 rue hec
mal`, and mimir returns possible candidates in a dropdown box.

![qwant maps](https://user-images.githubusercontent.com/3987698/56976025-53ed1180-6b72-11e9-9c81-9718e92061ce.png)

# Getting Started

For an introduction to the project, you can have a look at the [Introduction to
Mimirsbrunn](docs/tutorials/introduction.md).


These instructions will give you a copy of the project up and running on your local machine for
development and testing purposes. See deployment for notes on deploying the project on a live
system.

## Prerequisites

Mimirsbrunn is a rust project, so you need to have a rust development environment. Instructions on
how to do that are found on the [rust website](https://www.rust-lang.org/tools/install).

Additionally, mimirsbrunn relies on [cosmogony](https://github.com/osm-without-borders/cosmogony) to
create so called cosmogony files which are used for indexing administrative regions.

For running end to end or unit tests, you need the docker engine available.

## Installing

### Debian

You can install Mimirsbrunn from debian packages available as build artifacts on the repository
homepage. [FIXME Where are .deb ?]

### Docker

You can also fetch official images from DockerHub here:

 - [navitia/mimirsbrunn:latest](https://hub.docker.com/r/navitia/mimirsbrunn)
 - [navitia/bragi:latest](https://hub.docker.com/r/navitia/bragi)


If you want to get the commit ref used to build the latest image you can run the following command:

```bash
docker inspect --format='{{index .Config.Labels "org.label-schema.vcs-ref"}}' navitia/mimirsbrunn:latest
```

### Manually

You can build Mimirsbrunn manually, as the following instructions explain.

You need to retrieve the project and build it using the rust compiler:

```
git clone https://github.com/CanalTP/mimirsbrunn.git
cd mimirsbrunn
cargo build --release
```

## Running

Having built the project, you can now perform a sanity check. This will serve as a test that the
program works, and as a basic example of the two main functionalities: indexing and querying.

So we'll index administrative regions into Elasticsearch, and then search for some of them. We'll
focus on one country, lets say... Denmark!

1. Download OSM file.

First we download the OSM file for Denmark, from the
[geofabrik](http://download.geofabrik.de/europe/denmark-latest.osm.pbf) server, and store the file
locally.

2. Generate cosmogony file.

If you haven't installed cosmogony yet, you need to do so now, by following the instructions
[here](https://github.com/osm-without-borders/cosmogony). You can then transform the original
OSM PBF file for Denmark (<p class="callout warning"> The following command must be typed in
the directory of the cosmogony project)

```
cargo run --release -- generate -i /path/to/denmark-latest.osm.pbf -o denmark.jsonl.gz
```

3. Start an Elasticsearch docker container.

We'll start an Elasticsearch version 7.13, available on ports 9200 and 9300.

```
docker run --name mimirsbrunn-test \
  -p 9200:9200 -p 9300:9300 \
  -e "discovery.type=single-node" \
  -d docker.elastic.co/elasticsearch/elasticsearch:7.13.0
```

You can check that Elasticsearch has correctly started (maybe wait for about 10/20s for
Elasticsearch to be available):

```
curl 'http://localhost:9200'
{
  "name": "...",
  "cluster_name": "docker-cluster",
  ...
k  "tagline": "You Know, for Search"
}
```

4. Prepare Elasticsearch

We use templates, such that if you create an index that starts with a certain prefix, if a templates
is configured for that prefix, it will create the index with the values found in that template. So,
before creating any index, you need to import all the templates in Elasticsearch.

```
cd mimirsbrunn
./target/release/ctlmimir \
  -s "elasticsearch.url='http://localhost:9200'" \
  -c ./config \
  -m testing \
  run
```

You should see all the index templates by querying your Elasticsearch. For example, the template for
administrative regions:

```
curl 'http://localhost:9200/_index_template/mimir-admin*'
{
  "index_templates": [
    {
      "name": "mimir-admin",
      "index_template": {
        "index_patterns": [
          "munin_admin*"
        ],
        ...
        "composed_of": [
          "mimir-base",
          "mimir-dynamic-mappings"
        ],
        "priority": 10,
        "version": 3
      }
    }
  ]
}
```

5. Index cosmogony into Elasticsearch

The result of building the mimirsbrunn project includes several binaries located in
`/target/releases`, one of which is used to index cosmogony files:

cosmogony2mimir uses several configuration files found in the source code, and they work fine by
default. In the following command, we use a setting to make sure that cosmogony2mimir will target
the Elasticsearch container we just started: (See [here](/docs/indexings.md#cosmogony2mimir) for
more details about using cosmogony2mimir)

```
cd mimirsbrunn
./target/release/cosmogony2mimir \
  -c ./config \
  -m testing \
  -s "elasticsearch.url='http://localhost:9200'" \
  -s langs=['en', 'da'] \
  -i <path/to/denmark.jsonl.gz> \
  run
```

You can follow in the `mimirsbrunn/logs` directory.

5. Check Elasticsearch

The previous step created an index for administrative regions, and so you should be able to query your
Elasticsearch like so.

```
curl 'http://localhost:9200/_cat/indices'
health status index                                    uuid                   pri rep docs.count docs.deleted store.size pri.store.size
yellow open   munin_admin_fr_20211104_152535_346903898 FrWbs7PiRi26w-cbsIXjbg   1   4       1841            0     16.3mb         16.3mb
```

6. Start Bragi

7. Query Bragi

## Testing

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

## Where to Go from There

Maybe you find that some the results you get are not ranked correctly, and want to adjust the way
Elasticsearch is configured. So you may want to learn [how Elasticsearch templates are
configured](/docs/elasticsearch.md) and [how to get there](/docs/process/elasticsearch.md).

You want to know more about indexing data into Elasticsearch.

You want to know more about bragi and how to query Elasticsearch via mimir, start
[here](/docs/bragi.md).

# Development

You can find more developer oriented documentation [here](/docs/developer.md)

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process
for submitting pull requests to us.

## Versioning

We use [Semantic Versioning](http://semver.org/) for versioning. For the versions available, see the
[tags on this repository](https://github.com/CanalTP/mimirsbrunn/tags).

## Authors

Mimirsbrunn is a project initially started by [Guillaume Pinot](https://github.com/TeXitoi/) and
[Antoine Desbordes](https://github.com/antoine-de) for [Navitia](http://navitia.io).

See also the list of [contributors](https://github.com/CanalTP/mimirsbrunn/contributors) who
participated in this project.

# Processes

[processes](/docs/process/README.md)

# Who Uses It ?

* [Navitia](https://github.com/CanalTP/navitia)
* [Qwant Maps](https://www.qwant.com/maps)

If you use it too, feel free to open a pull request, we'll be happy to add your project here!

# Alternatives

* [pelias](https://github.com/pelias/pelias)
* [photon](https://github.com/komoot/photon)
* [addok](https://github.com/addok/addok)

TODO: add a bit more detail on all the projects

All those projects use quite the same APIs, and you can compare their results using
[geocoder-tester](https://github.com/geocoders/geocoder-tester).

For a more visual comparison, you can also use [a comparator](https://github.com/CanalTP/autocomplete-comparator).

# Resources

* [A french presentation of Mimirsbrunn](https://github.com/TeXitoi/pinot2017bano/blob/master/pinot2017bano.pdf)

# License

This project is licensed under the [AGPLv3](LICENSE.md) GNU Affero General Public License - see the
[LICENSE.md](LICENSE.md) file for details

# Acknowledgments

  - **Billie Thompson** - *Provided README Template* - [PurpleBooth](https://github.com/PurpleBooth)


