# Indexing

Indexing is the process by which we take data sources and store them as indexes
in Elasticsearch. This process involves several steps of parsing, validating,
and enriching the data prior to the actual indexing.

## Getting Started

Indexing is done with several binaries that are built within the Mimirsbrunn project. Assuming you
have a rust environment, you can just do the following:

```
git clone git@github.com:hove-io/mimirsbrunn.git
cd mimirsbrunn
cargo build --release
```

This will create several executable in `./target/release/{osm2mimir, cosmogony2mimir, ...}`

## Usage

The following table shows what binary and source of data you need depending on the type of data
you want to index:

<table>
<colgroup>
<col style="width: 22%" />
<col style="width: 47%" />
<col style="width: 29%" />
</colgroup>
<thead>
<tr class="header">
<th>type</th>
<th>binary</th>
<th>data source</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>administrative regions (admins)</td>
<td><a href="#cosmogony2mimir">cosmogony2mimir</a></td>
<td><a href="#cosmogony">cosmogony</a></td>
</tr>
<tr class="even">
<td>streets</td>
<td><a href="#osm2mimir">osm2mimir</a></td>
<td><a href="#OSM">OSM</a></td>
</tr>
<tr class="odd">
<td>addresses</td>
<td><p><a href="#bano2mimir">bano2mimir</a></p>
<p><a href="#openaddress2mimir">openaddress2mimir</a></p></td>
<td><p><a href="#BANO">BANO</a> (France)</p>
<p>OpenAddresses</p></td>
</tr>
<tr class="even">
<td>public transport stop locations</td>
<td><a href="#ntfs2mimir">ntfs2mimir</a></td>
<td><a href="#NTFS">NTFS</a></td>
</tr>
<tr class="odd">
<td>public points of interests (POI)</td>
<td><a href="#osm2mimir">osm2mimir</a></td>
<td><a href="#OSM">OSM</a></td>
</tr>
<tr class="even">
<td>private points of interests (POI)</td>
<td><a href="#poi2mimir">poi2mimir</a></td>
<td></td>
</tr>
</tbody>
</table>

If you want to index all of those data types for a given geographical regions, your process will 
involve the following, which will be described below:

1. Download the datasets
2. Launch an Elasticsearch
3. Launch indexing binaries in a specific order

There is currently two tools that will help you wrap these steps in an easy and configurable manner:

* [Fafnir](https://github.com/QwantResearch/fafnir)
* import2mimir is a bash script 

### Download datasets

#### cosmogony

cosmogony datasets are produced by a binary cosmogony available from
[here](https://github.com/osm-without-borders/cosmogony), which uses an OSM pbf
input.

cosmogony is a rust project, so you need to build it first (these commands are
taken from cosmogony's README, so you may want to see for updates first)

```
curl https://sh.rustup.rs -sSf | sh    # intall rust
apt-get install libgeos-dev            # install GEOS
git clone https://github.com/osm-without-borders/cosmogony.git     # Clone this repo
cd cosmogony;                          # enter the directory
git submodule update --init            # update the git submodules
cargo build --release                  # finally build cosmogony
```

cosmogony runs on OSM pbf documents, which you need to download. Once you have
a dataset, you can run

```
./target/release/cosmogony -i <path/to/source.osm.pbf> -o <path/to/output.ext>
```

The output extension can be any of {`json`, `jsonl`, `json.gz`, `jsonl.gz`}

#### OSM

You can download OpenStreetMap data for different regions of the world from
[geofabrik](http://download.geofabrik.de/). Be sure to download no more than
you need. Larger files will take longer to index, and will take more memory in
Elasticsearch.

#### BANO

You can download BANO datasets from [Openstreetmap](http://bano.openstreetmap.fr)

#### NTFS

You can download NTFS datasets from [Navitia](https://navitia.opendatasoft.com)

### Launch Elasticsearch

You can either benefits from a full blown elasticsearch deployment, or, for
evaluation purposes, use a docker container. Here is the command that will
create and run such a container:

```
docker run -p 9200:9200 -p 9300:9300 \
  -e "discovery.type=single-node" --name "elasticsearch" \
  -d docker.elastic.co/elasticsearch/elasticsearch:7.13.0
```

### Indexing Data

We mentioned earlier that your data is parsed, validated, and enriched before
beeing indexed into Elasticsearch. So processing the data may require to
extract previously indexed data from Elasticsearch to add context to a
geospatial place. For example, if we index an address from BANO, we may want to
add a reference to a street and to the administrative region it belongs to.

Here is the order of execution:
1. `cosmogony2mimir`
2. `osm2mimir`
3. `bano2mimir` / `openaddress2mimir`
4. `ntfs2mimir`
5. `poi2mimir`

These binaries follow the same pattern for configuration and command line, and this is detailed
in a later [section](#configuring_indexing).

These binaries have a subcommand at the end, which can be either
* *run*: execute the program
* *config*: print the configuration as a json file.

### cosmogony2mimir

As mentioned earlier, `cosmogony2mimir` is the binary responsible for indexing administrative
regions into Elasticsearch. Here is a usage example:

```
cosmogony2mimir -m testing -s elasticsearch.url='http://localhost:9204' -c ./config -i idf.jsonl.gz run
```

It will read the default and testing configuration in
`./config/cosmogony2mimir` and `./config/elasticsearch`, override the
elasticsearch url, and read its input from  `idf.jsonl.gz`

cosmogony2mimir will check the following files (or more, depending on its mode, see
[below](#configuring-indexing) )

<table>
<colgroup>
<col style="width: 55%" />
<col style="width: 44%" />
</colgroup>
<thead>
<tr class="header">
<th>configuration path</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td><code>&lt;base-config&gt;/elasticsearch/default.toml</code></td>
<td>Connection to Elasticsearch</td>
</tr>
<tr class="even">
<td><code>&lt;base-config&gt;/cosmogony2mimir/default.toml</code></td>
<td>Cosmogony Specific + logging + dataset</td>
</tr>
<tr class="odd">
<td><code>&lt;base-config&gt;/elasticsearch/admin/mappings.json</code></td>
<td>Elasticsearch mappings for admins</td>
</tr>
<tr class="even">
<td><code>&lt;base-config&gt;/elasticsearch/admin/settings.json</code></td>
<td>Elasticsearch settings for admins</td>
</tr>
</tbody>
</table>

The specific configuration for cosmogony2mimir includes:

<table>
<colgroup>
<col style="width: 18%" />
<col style="width: 16%" />
<col style="width: 45%" />
<col style="width: 20%" />
</colgroup>
<thead>
<tr class="header">
<th>configuration key</th>
<th>type</th>
<th>description</th>
<th>example</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>langs</td>
<td>array of string</td>
<td>list of language (ISO 639-2 Code) used to index administrative regions</td>
<td><code>langs=['fr', 'en']</code></td>
</tr>
</tbody>
</table>

### osm2mimir

`osm2mimir` indexes streets and public POIs into Elasticsearch. You need to have indexed
administrative regions into the same Elasticsearch first. The command line follows the same pattern
as the other binaries:

```
osm2mimir -m testing -s elasticsearch.url='http://localhost:9204' -c ./config -i idf.osm.pbf run
```

Depending on its configuration (eg if both pois and streets are imported), osm2mimir will
need the following configuration files (or more, depending on its mode, see
[below](#configuring-indexing) )

<table>
<colgroup>
<col style="width: 56%" />
<col style="width: 43%" />
</colgroup>
<thead>
<tr class="header">
<th>configuration path</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td><code>&lt;base-config&gt;/elasticsearch/default.toml</code></td>
<td>Connection to Elasticsearch</td>
</tr>
<tr class="even">
<td><code>&lt;base-config&gt;/osm2mimir/default.toml</code></td>
<td>osm2mimir Specific + logging + dataset</td>
</tr>
<tr class="odd">
<td><code>&lt;base-config&gt;/elasticsearch/poi/mappings.json</code></td>
<td>Elasticsearch mappings for pois</td>
</tr>
<tr class="even">
<td><code>&lt;base-config&gt;/elasticsearch/poi/settings.json</code></td>
<td>Elasticsearch settings for pois</td>
</tr>
<tr class="odd">
<td><code>&lt;base-config&gt;/elasticsearch/street/mappings.json</code></td>
<td>Elasticsearch mappings for streets</td>
</tr>
<tr class="even">
<td><code>&lt;base-config&gt;/elasticsearch/street/settings.json</code></td>
<td>Elasticsearch settings for streets</td>
</tr>
</tbody>
</table>

The specific configuration for osm2mimir includes the following parameters:

<table>
<colgroup>
<col style="width: 20%" />
<col style="width: 17%" />
<col style="width: 26%" />
<col style="width: 35%" />
</colgroup>
<thead>
<tr class="header">
<th>configuration key</th>
<th>type</th>
<th>description</th>
<th>example</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>pois.import</td>
<td>boolean</td>
<td>Indicate if osm2mimir indexes pois</td>
<td><code>pois.import=true</code></td>
</tr>
<tr class="even">
<td>pois.config.types</td>
<td>array of tables</td>
<td></td>
<td><pre><code>pois.config.types=[{
id = &quot;poi_type:amenity:parking&quot;,
name = &quot;parking&quot; }]</code></pre></td>
</tr>
<tr class="odd">
<td>pois.config.rules</td>
<td>array of tables</td>
<td></td>
<td><pre><code>pois.config.rules=[{
type = &quot;poi_type:amenity:parking&quot;,
filters = [{
key = &quot;amenity&quot;,
value = &quot;parking&quot;
}]
}]</code></pre></td>
</tr>
<tr class="even">
<td>streets.import</td>
<td>boolean</td>
<td>Indicate if osm2mimir indexes streets</td>
<td><code>streets.import=true</code></td>
</tr>
<tr class="odd">
<td>streets.exclusions</td>
<td>table</td>
<td>Indicate what objects are not indexed.</td>
<td><pre><code>streets.exclusions={
highways=[&quot;elevator&quot;, &quot;escape&quot;],
public_transport=[&quot;platform&quot;]
}</code></pre></td>
</tr>
</tbody>
</table>

### bano2mimir

### openaddress2mimir

### ntfs2mimir

### poi2mimir

### import2mimir

This is a script which is intended to make it easy to play with a simple dataset.

### Configuring Indexing

#### A layering process

For configuring indexing binaries, we use the crate
[config](https://crates.io/crates/config), which enables a layered
configuration, and so the process of getting a configuration is as follow:

1. We start with a default configuration. For example, for configuring the elasticsearch connection,
	 that means we read `config/elasticsearch/default.toml`.
2. If the user specifies a run mode, then we read the corresponding file. If you are in production,
	 and you specify the `--run-mode prod`, then we will go read, for elasticsearch,
	 `config/elasticsearch/prod.toml`. These values override the default values set in (1).
3. Then a `config/elasticsearch/local.toml` file can be used to override some values again.
4. Then, environment variables can be used on top of the previous values: The name of the
	 environment variable is all in upper case: It is the concatenation of a prefix, followed by an
	 underscore, followed by the path to the value, separated by underscores. So if you want to change
	 the `elasticsearch.url` value for osm2mimir, you'd use, for example,
	 `MIMIR_ELASTICSEARCH_URL=http://localhost:9999`
5. Finally, you can still override some values with the commandline, by using `--setting
	 elasticsearch.url='http://localhost:9999'`. You use the format (`<key>=<value>`), where
	 the value must be written in a valid TOML syntax: For example, to set an array of strings,
	 you would use `--setting lang=['fr', 'de', 'es']`.

This way of configuring allows great flexibility, and also a very simple generic command line.
It can also be a bit tricky to know what the exact final configuration will be. All binaries
have a `config` subcommand, which displays the configuration in json format.

#### Filesystem layout

All the configuration stored with the code is found in the `config` directory
at the base of the project. 

Under `config`, you'll have a directory for `elasticsearch` because its shared
by all binaries, and we don't want to repeat elasticsearch configuration for
all the binaries. In Elasticsearch, you have one folder for each place, ie
*admin*, *address*, … . Each of this folder is to configure the related index
mapping and setting. Under the elasticsearch folder, you also have a
configuration file, `default.toml` to define the connection parameters.

```
config
  ├─── elasticsearch
  ┊      ├─── admins
         │      ├─── settings.json
         │      └─── mappings.json
         ├─── streets
         │      ├─── settings.json
         │      └─── mappings.json
         ┊
         │
         ├─── default.toml
         ├─── testing.toml
         └─── prod.toml
```

You also have binary specific configuration: For example, for indexing stops, you use
`ntfs2mimir`. So you'll find `ntfs2mimir` related configuration in a folder `config/ntfs2mimir`.

```
config
  ┊
  ├─── ntfs2mimir
         ├─── default.toml
         └─── testing.toml
```

