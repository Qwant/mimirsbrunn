## Components: Import Tools & Web Service

There are several components in Mimirsbrunn. Most of them are dedicated to the import of data while other are web services ([bragi](#bragi)) to wrap Elasticsearch interactions.
All the Mimirsbrunn's components described below implement the `--help` (or `-h`) argument to explain their use.

### Import Tools

Before using [Bragi](#bragi), you have to import data into Elasticsearch.
The default and easiest way to import data is to use the [docker_mimir](https://github.com/QwantResearch/docker_mimir) tool.
However the following import tools are still possible.

**First** you have to import admins objects. You can load them from Cosmogony or from OSM. Cosmogony give better results. Use `cosmogony2mimir` or `osm2mimir --import-admin`.

#### cosmogony2mimir

- This tool imports [Cosmogony](https://github.com/osm-without-borders/cosmogony/) data into Mimir. Cosmogony data are generated from OSM and brings geographical zones with a structured hierarchy.

You first needs to generate cosmogony data (as for the moment there are none already available for download).

```shell
<path to cosmogony executable> --input=planet-latest.osm.pbf --output=cosmogony.jsonl.gz
```

Then to import all those data into Mimir, you only have to do:
```shell
cargo run --release --bin cosmogony2mimir -- --input=cosmogony.jsonl.gz --connection-string=http://localhost:9200
```

#### osm2mimir

- This tool imports OpenStreetMap data into Mimir. It is recommended to run osm integration **after** [Cosmogony](https://github.com/osm-without-borders/cosmogony) integration in order to attach the objects to admins. You can get OpenStreetMap data from [Geofabrik](http://download.geofabrik.de/), for instance:
```shell
curl -O http://download.geofabrik.de/europe/france-latest.osm.pbf
```
- Then to import all those data into Mimir, you only have to do:
```shell
cargo run --release --bin osm2mimir -- --input=france-latest.osm.pbf --import-way --import-poi --connection-string=http://localhost:9200
```

#### bano2mimir

- This tool imports bano's data into Mimir. It is recommended to run bano integration **after** [Cosmogony](https://github.com/osm-without-borders/cosmogony) integration in order to attach addresses to admins. You can get bano's data from [OpenStreetMap](http://bano.openstreetmap.fr/data/), for instance:
```shell
curl -O http://bano.openstreetmap.fr/data/full.csv.gz
gunzip full.csv.gz
```
- To import all those data into Mimir, you only have to do:
```shell
cargo run --release --bin bano2mimir -- --input full.csv --connection-string=http://localhost:9200/
```

#### ntfs2mimir

- This tool imports data from the ntfs files into Mimir. It is recommended to run ntfs integration **after** [Cosmogony](https://github.com/osm-without-borders/cosmogony) integration so that stops are attached to admins. You can get these data from [Navitia](https://navitia.opendatasoft.com/explore).

- To import all those data into Mimir, you only have to do:
```shell
cargo run --release --bin ntfs2mimir -- -i <path_to_folder_with_ntfs_file> --dataset=idf --connection-string=http://localhost:9200/
```

- The ntfs input file needs to match the [NTFS specification](https://github.com/hove-io/navitia/blob/dev/documentation/ntfs/ntfs_0.6.md).

#### stops2mimir

- This import tool is still available but is now deprecated because ntfs2mimir already imports stops.

### <a name=bragi> Web Service: Bragi </a>

Bragi is the webservice built around ElasticSearch.
Its purpose is to hide the ElasticSearch complexity and to return consistent formated responses.

Its responses format follow the [geocodejson-spec](https://github.com/geocoders/geocodejson-spec).

This is a format used by other geocoding API such as [Addok](https://github.com/addok/addok) or [Photon](https://github.com/komoot/photon).

- To run Bragi:
```shell
cargo run --release --bin bragi -- --connection-string=http://localhost:9200/munin
```

- Then you can call the API (the default Bragi's listening port is 4000):
```shell
curl "http://localhost:4000/autocomplete?q=rue+hector+malot"
```
