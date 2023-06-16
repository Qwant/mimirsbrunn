Elasticsearch Process
=====================

  * [Creating Templates](#creating-templates)
    * [Gathering Fields](#gathering-fields)
      * [Administrative Region](#administrative-region)
      * [Address](#address)
      * [Street](#street)
      * [POI](#point-of-interest)
      * [Stop](#stop)
    * [Partitioning Templates](#partitioning-templates)
      * [Common Templates](#common-templates)
      * [Index Templates](#index-templates)
      * [Search Templates](#search-templates)
  * [Using Templates](#using-templates)
    * [Importing Templates](#importing-templates)
    * [Overriding Templates](#overriding-templates)
  * [Updating Templates](#updating-templates)
    * [Evaluating Templates](#evaluating-templates)

This document describes the process of configuring Elasticsearch templates for Mimirsbrunn.

We can picture Elasticsearch as a black box, where we store JSON documents. These documents are of
different kinds, and depend on our business. Since we deal with geospatial data, and Navitia in
particular works with public transportations, the types of documents we store are:

* administrative regions: 
* addresses: 
* streets
* point of interests (POIs)
* stops (Public Transportations)

We first submit configuration files to Elasticsearch to describe how we want each document type to
be handled. These are so called component templates, and index templates, which include:
* settings: how do we want the text to be handled? do we want to use synonyms, lowercase, use stems,…
* mappings: how each field of each type of document listed above is handled.

When the documents are indexed according to our settings and mappings, we can then query
Elasticsearch, and play with lots of parameters to push the ranking of documents up or down.

This document describes how we establish a baseline for these templates, and the process of updating
them.

Configuring Elasticsearch templates is an iterative process, which, when done right, results in:
* reduced memory consumption in Elasticsearch, by reducing the size / number of indices.
* reduced search duration, by simplifying the query
* better ranking

# Creating Templates

## Gathering Fields

We'll construct a table with all the fields, for each type of document. The source of information is
the document, which is a rust structure serialized to JSON. When building this resource, be sure to
exclude what would be skipped (marked as `skip`) by the serializer.

### <a id="administrative-regions-fields"></a> [Administrative Region](/crates/libs/places/src/admin.rs)

<table>
<colgroup>
<col style="width: 19%" />
<col style="width: 21%" />
<col style="width: 58%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>administrative_regions</td>
<td>Vec&lt;Arc<Admin>&gt;</td>
<td>A list of parent administrative regions</td>
</tr>
<tr class="even">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td>Coordinates of (the center??) of the region, similar to coord Given in lat lon</td>
</tr>
<tr class="odd">
<td>bbox</td>
<td>Option&lt;Rect<f64>&gt;</td>
<td>Bounding Box</td>
</tr>
<tr class="even">
<td>boundary</td>
<td>Option&lt;MultiPolygon<f64>&gt;</td>
<td>Describes the shape of the admin region</td>
</tr>
<tr class="odd">
<td>codes</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td>Some codes used in OSM, like ISO3166, ref:nuts, wikidata</td>
</tr>
<tr class="even">
<td>context</td>
<td>Option<Context></td>
<td>Used for debugging</td>
</tr>
<tr class="odd">
<td>coord</td>
<td>Coord</td>
<td>Coordinates of the region</td>
</tr>
<tr class="even">
<td>country_codes</td>
<td>Vec<String></td>
<td>Country Codes</td>
</tr>
<tr class="odd">
<td>id</td>
<td>String</td>
<td>Unique id created by cosmogony</td>
</tr>
<tr class="even">
<td>insee</td>
<td>String</td>
<td>A code used to identify regions in France. From <a href="https://wiki.openstreetmap.org/wiki/Key:ref:INSEE?uselang=en">OSM</a></td>
</tr>
<tr class="odd">
<td>label</td>
<td>String</td>
<td>??</td>
</tr>
<tr class="even">
<td>labels</td>
<td>I18nProperties</td>
<td>??</td>
</tr>
<tr class="odd">
<td>level</td>
<td>u32</td>
<td>Position of the region in the admin hierarchy</td>
</tr>
<tr class="even">
<td>name</td>
<td>String</td>
<td>Name</td>
</tr>
<tr class="odd">
<td>names</td>
<td>I18nProperties</td>
<td>Name, but internationalized, eg name:en, name:ru, name:es</td>
</tr>
<tr class="even">
<td>parent_id</td>
<td>Option<String></td>
<td>id of the parent admin region (or none if root)</td>
</tr>
<tr class="odd">
<td>weight</td>
<td>f64</td>
<td>A number associated with the population in that region</td>
</tr>
<tr class="even">
<td>zip_codes</td>
<td>Vec<String></td>
<td>Zip codes (can be more than one)</td>
</tr>
<tr class="odd">
<td>zone_type</td>
<td>Option<ZoneType></td>
<td>Describes the type, eg city, suburb, country,…</td>
</tr>
</tbody>
</table>

### <a id="addresses-fields"></a> [Address](/crates/libs/places/src/addr.rs)

Addresses, compared to administrative regions, have very little unique fields, just house number and
street:

<table>
<colgroup>
<col style="width: 18%" />
<col style="width: 22%" />
<col style="width: 58%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td></td>
</tr>
<tr class="even">
<td>context</td>
<td>Option<Context></td>
<td></td>
</tr>
<tr class="odd">
<td>coord</td>
<td>Coord</td>
<td></td>
</tr>
<tr class="even">
<td>country_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
<tr class="odd">
<td>house_number</td>
<td>String</td>
<td>Identifier in the street</td>
</tr>
<tr class="even">
<td>id</td>
<td>String</td>
<td>Unique identifier</td>
</tr>
<tr class="odd">
<td>label</td>
<td>String</td>
<td></td>
</tr>
<tr class="even">
<td>name</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>street</td>
<td>Street</td>
<td>Reference to the street the address belongs to.</td>
</tr>
<tr class="even">
<td>weight</td>
<td>f64</td>
<td></td>
</tr>
<tr class="odd">
<td>zip_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
</tbody>
</table>

### [Street](/crates/libs/places/src/street.rs)

No particular fields for streets:

<!-- docs/assets/tbl/fields-street.md -->

<table style="width:81%;">
<colgroup>
<col style="width: 34%" />
<col style="width: 26%" />
<col style="width: 19%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>administrative_regions</td>
<td>Vec&lt;Arc<Admin>&gt;</td>
<td></td>
</tr>
<tr class="even">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td></td>
</tr>
<tr class="odd">
<td>context</td>
<td>Option<Context></td>
<td></td>
</tr>
<tr class="even">
<td>coord</td>
<td>Coord</td>
<td></td>
</tr>
<tr class="odd">
<td>country_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
<tr class="even">
<td>id</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>label</td>
<td>String</td>
<td></td>
</tr>
<tr class="even">
<td>name</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>weight</td>
<td>f64</td>
<td></td>
</tr>
<tr class="even">
<td>zip_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
</tbody>
</table>

### <a id="pois-fields"></a> [Point of Interest](/crates/libs/places/src/poi.rs)

<!-- docs/assets/tbl/fields-poi.md -->

<table>
<colgroup>
<col style="width: 28%" />
<col style="width: 31%" />
<col style="width: 40%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>address</td>
<td>Option
<Address></td>
<td>Address associated with that POI Can be an address or a street</td>
</tr>
<tr class="even">
<td>administrative_regions</td>
<td>Vec&lt;Arc<Admin>&gt;</td>
<td></td>
</tr>
<tr class="odd">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td></td>
</tr>
<tr class="even">
<td>context</td>
<td>Option<Context></td>
<td></td>
</tr>
<tr class="odd">
<td>coord</td>
<td>Coord</td>
<td></td>
</tr>
<tr class="even">
<td>country_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
<tr class="odd">
<td>id</td>
<td>String</td>
<td></td>
</tr>
<tr class="even">
<td>label</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>labels</td>
<td>I18nProperties</td>
<td></td>
</tr>
<tr class="even">
<td>name</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>names</td>
<td>I18nProperties</td>
<td></td>
</tr>
<tr class="even">
<td>poi_type</td>
<td>PoiType</td>
<td>id / name references in NTFS</td>
</tr>
<tr class="odd">
<td>properties</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td></td>
</tr>
<tr class="even">
<td>weight</td>
<td>f64</td>
<td></td>
</tr>
<tr class="odd">
<td>zip_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
</tbody>
</table>

### <a id="stops-fields"></a> [Stop](/crates/libs/places/src/stop.rs) (Public Transportations)

<!-- docs/assets/tbl/fields-stop.md -->

<table>
<colgroup>
<col style="width: 24%" />
<col style="width: 26%" />
<col style="width: 48%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>administrative_regions</td>
<td>Vec&lt;Arc<Admin>&gt;</td>
<td></td>
</tr>
<tr class="even">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td></td>
</tr>
<tr class="odd">
<td>codes</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td></td>
</tr>
<tr class="even">
<td>comments</td>
<td>Vec<Comment></td>
<td></td>
</tr>
<tr class="odd">
<td>commercial_modes</td>
<td>Vec<CommercialMode></td>
<td></td>
</tr>
<tr class="even">
<td>context</td>
<td>Option<Context></td>
<td></td>
</tr>
<tr class="odd">
<td>coord</td>
<td>Coord</td>
<td></td>
</tr>
<tr class="even">
<td>country_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
<tr class="odd">
<td>coverages</td>
<td>Vec<String></td>
<td></td>
</tr>
<tr class="even">
<td>feed_publishers</td>
<td>Vec<FeedPublisher></td>
<td></td>
</tr>
<tr class="odd">
<td>id</td>
<td>String</td>
<td></td>
</tr>
<tr class="even">
<td>label</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>lines</td>
<td>Vec<Line></td>
<td></td>
</tr>
<tr class="even">
<td>name</td>
<td>String</td>
<td></td>
</tr>
<tr class="odd">
<td>physical_modes</td>
<td>Vec<PhysicalMode></td>
<td></td>
</tr>
<tr class="even">
<td>properties</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td></td>
</tr>
<tr class="odd">
<td>timezone</td>
<td>String</td>
<td></td>
</tr>
<tr class="even">
<td>weight</td>
<td>f64</td>
<td>The weight depends on the number of lines, and other parameters.</td>
</tr>
<tr class="odd">
<td>zip_codes</td>
<td>Vec<String></td>
<td></td>
</tr>
</tbody>
</table>

## Partitioning Templates

When we combine together all the fields from the previous documents, we obtain the following table,
which shows all the fields in use, and by what type of document.

<!-- docs/assets/tbl/fields.md -->

<table>
<colgroup>
<col style="width: 17%" />
<col style="width: 19%" />
<col style="width: 41%" />
<col style="width: 4%" />
<col style="width: 4%" />
<col style="width: 4%" />
<col style="width: 4%" />
<col style="width: 4%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>description</th>
<th>adm</th>
<th>add</th>
<th>poi</th>
<th>stp</th>
<th>str</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>address</td>
<td>Option
<Address></td>
<td>Address associated with that POI</td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>administrative_regions</td>
<td>Vec&lt;Arc<Admin>&gt;</td>
<td>A list of parent administrative regions</td>
<td>✓</td>
<td></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="odd">
<td>approx_coord</td>
<td>Option<Geometry></td>
<td>Coordinates of the object, similar to coord</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>bbox</td>
<td>Option&lt;Rect<f64>&gt;</td>
<td>Bounding Box</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="odd">
<td>boundary</td>
<td>Option&lt;MultiPolygon<f64>&gt;</td>
<td>Describes the shape of the admin region</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>codes</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td>Some codes used in OSM, like ISO3166, ref:nuts, wikidata</td>
<td>✓</td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>comments</td>
<td>Vec<Comment></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="even">
<td>commercial_modes</td>
<td>Vec<CommercialMode></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>context</td>
<td>Option&lt;Conte✓t&gt;</td>
<td>Used to return information (debugging)</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>coord</td>
<td>Coord</td>
<td></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="odd">
<td>country_codes</td>
<td>Vec<String></td>
<td>Country Codes</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>coverages</td>
<td>Vec<String></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>feed_publishers</td>
<td>Vec<FeedPublisher></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="even">
<td>house_number</td>
<td>String</td>
<td>Identifier in the street</td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="odd">
<td>id</td>
<td>String</td>
<td>Unique identifier</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>insee</td>
<td>String</td>
<td>A code used to identify regions in France.</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="odd">
<td>label</td>
<td>String</td>
<td>??</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>labels</td>
<td>I18nProperties</td>
<td>??</td>
<td>✓</td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
</tr>
<tr class="odd">
<td>level</td>
<td>u32</td>
<td>Position of the region in the admin hierarchy</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>lines</td>
<td>Vec<Line></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>name</td>
<td>String</td>
<td>Name</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>names</td>
<td>I18nProperties</td>
<td>Name, but internationalized, eg name:en, name:ru, name:es</td>
<td>✓</td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
</tr>
<tr class="odd">
<td>parent_id</td>
<td>Option<String></td>
<td>id of the parent admin region (or none if root)</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>physical_modes</td>
<td>Vec<PhysicalMode></td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>poi_type</td>
<td>PoiType</td>
<td>id / name references in NTFS</td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>properties</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>street</td>
<td>Street</td>
<td>Reference to the street the address belongs to.</td>
<td></td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
</tr>
<tr class="even">
<td>timezone</td>
<td>String</td>
<td></td>
<td></td>
<td></td>
<td></td>
<td>✓</td>
<td></td>
</tr>
<tr class="odd">
<td>weight</td>
<td>f64</td>
<td></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="even">
<td>zip_codes</td>
<td>Vec<String></td>
<td></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
</tr>
<tr class="odd">
<td>zone_type</td>
<td>Option<ZoneType></td>
<td>Describes the type, eg city, suburb, country,…</td>
<td>✓</td>
<td></td>
<td></td>
<td></td>
<td></td>
</tr>
</tbody>
</table>

Talk about `type`, `indexed_at` (and pipeline)

### Component Templates

We can extract from this table a list of fields that are (almost) common to all the documents. In
this table of common fields, we indicate what type is used for Elasticsearch, whether we should
index the field, and some comments.

<!-- docs/assets/tbl/fields-common.md -->

<table style="width:100%;">
<colgroup>
<col style="width: 16%" />
<col style="width: 13%" />
<col style="width: 3%" />
<col style="width: 3%" />
<col style="width: 3%" />
<col style="width: 3%" />
<col style="width: 3%" />
<col style="width: 10%" />
<col style="width: 5%" />
<col style="width: 34%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>adm</th>
<th>add</th>
<th>poi</th>
<th>stp</th>
<th>str</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>administrative_regions</td>
<td><code>Vec&lt;Arc&lt;Admin&gt;&gt;</code></td>
<td>✓</td>
<td></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td></td>
<td>✗</td>
<td>large object</td>
</tr>
<tr class="even">
<td>approx_coord</td>
<td><code>Option&lt;Geometry&gt;</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>??</td>
<td>✗</td>
<td>Improved geo_point in Elasticsearch may render <code>approx_coord</code> obsolete</td>
</tr>
<tr class="odd">
<td>context</td>
<td><code>Option&lt;Context&gt;</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td></td>
<td>✗</td>
<td>Output</td>
</tr>
<tr class="even">
<td>coord</td>
<td><code>Coord</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>geo_point</td>
<td>✓</td>
<td>Index for reverse API</td>
</tr>
<tr class="odd">
<td>country_codes</td>
<td><code>Vec&lt;String&gt;</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>??</td>
<td>✗</td>
<td>Are we searching with these ?</td>
</tr>
<tr class="even">
<td>id</td>
<td><code>String</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>keyword</td>
<td>✓</td>
<td>Index for features API. <strong>Really need to index??</strong></td>
</tr>
<tr class="odd">
<td>label</td>
<td><code>String</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>SAYT</td>
<td>✓</td>
<td>Field created by binaries (contains name and other informations, like admin, country code, …)</td>
</tr>
<tr class="even">
<td>name</td>
<td><code>String</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>text</td>
<td>✓</td>
<td>copy to <code>full label</code></td>
</tr>
<tr class="odd">
<td>weight</td>
<td><code>f64</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>float</td>
<td>✗</td>
<td>used for ranking</td>
</tr>
<tr class="even">
<td>zip_codes</td>
<td><code>Vec&lt;String&gt;</code></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>text</td>
<td>??</td>
<td>copy to <code>full label</code></td>
</tr>
</tbody>
</table>

Now we'll turn this table into an actual [component
template](/config/elasticsearch/templates/components/mimir-base.json), responsible for handling all
the common fields.

A few points are important to notice:
* The text based search is happening on the label. The label is created by the indexing program, and
  contains the name, some information about the administrative region it belongs to, maybe a
  country code. So we're not indexing the name, because the search is happening on the label.

The component template also contains additional fields, that are not present in the document sent by
the binaries:

<!-- docs/assets/tbl/fields-common-additional.md -->

<table>
<colgroup>
<col style="width: 11%" />
<col style="width: 5%" />
<col style="width: 5%" />
<col style="width: 5%" />
<col style="width: 5%" />
<col style="width: 5%" />
<col style="width: 5%" />
<col style="width: 16%" />
<col style="width: 6%" />
<col style="width: 34%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>adm</th>
<th>add</th>
<th>poi</th>
<th>stp</th>
<th>str</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>indexed_at</td>
<td><ul>
<li></li>
</ul></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>date</td>
<td>✗</td>
<td>Generated by an Elasticsearch pipeline</td>
</tr>
<tr class="even">
<td>type</td>
<td><ul>
<li></li>
</ul></td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>✓</td>
<td>constant_keyword</td>
<td>✗</td>
<td>Set in individual index templates</td>
</tr>
</tbody>
</table>

The search template has to reflect the information found in the common template.

### Index Templates

#### <a id="administrative-regions-template"></a> Admin

If we look back at the [list of fields](#administrative-regions-fields) present in the
administrative region document, and remove all the fields that are part of the common template, we
have the following list of remaining fields:

<!-- docs/assets/tbl/fields-2-admin.md -->

<table>
<colgroup>
<col style="width: 12%" />
<col style="width: 31%" />
<col style="width: 17%" />
<col style="width: 8%" />
<col style="width: 29%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>bbox</td>
<td><code>Option&lt;Rect&lt;f64&gt;&gt;</code></td>
<td>Bounding Box</td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>boundary</td>
<td><code>Option&lt;MultiPolygon&lt;f64&gt;&gt;</code></td>
<td>geo_shape</td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>codes</td>
<td><code>BTreeMap&lt;String, String&gt;</code></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>insee</td>
<td><code>String</code></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>labels</td>
<td><code>I18nProperties</code></td>
<td>??</td>
<td>✓</td>
<td>used in dynamic templates</td>
</tr>
<tr class="even">
<td>level</td>
<td><code>u32</code></td>
<td></td>
<td>✗</td>
<td>used for ranking</td>
</tr>
<tr class="odd">
<td>names</td>
<td><code>I18nProperties</code></td>
<td></td>
<td>✓</td>
<td>used in dynamic templates</td>
</tr>
<tr class="even">
<td>parent_id</td>
<td><code>Option&lt;String&gt;</code></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>zone_type</td>
<td><code>Option&lt;ZoneType&gt;</code></td>
<td>keyword</td>
<td>✓</td>
<td>used for filtering</td>
</tr>
</tbody>
</table>

The treatment of labels and names is done in a separate template, using dynamic templates.

This leaves the remaining fields to be indexed with the
[mimir-admin.json](/config/elasticsearch/templates/indices/mimir-admin.json) index template.

#### <a id="addresses-template"></a> Address

If we look back at the [list of fields](#addresses-fields) present in the administrative region
document, and remove all the fields that are part of the common template, we have the following list
of remaining fields:

<!-- docs/assets/tbl/fields-2-addr.md -->

<table style="width:100%;">
<colgroup>
<col style="width: 13%" />
<col style="width: 8%" />
<col style="width: 46%" />
<col style="width: 7%" />
<col style="width: 24%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>house_number</td>
<td>String</td>
<td>text</td>
<td>✓</td>
<td>?? Should we index it ?</td>
</tr>
<tr class="even">
<td>street</td>
<td>Street</td>
<td>Reference to the street the address belongs to.</td>
<td>✗</td>
<td></td>
</tr>
</tbody>
</table>

This leaves the remaining fields to be indexed with the
[mimir-addr.json](/config/elasticsearch/templates/indices/mimir-addr.json) index template.

#### <a id="streets-template"></a> Streets

For streets, its quite easy, because all the documents can be indexed with the base template,
leaving [mimir-street.json](/config/elasticsearch/templates/indices/mimir-street.json) index
template.

#### <a id="pois-template"></a> POIs

If we look back at the [list of fields](#pois-fields) present in the poi document, and remove all
the fields that are part of the common template, we have the following list of remaining fields:

<!-- docs/assets/tbl/fields-2-poi.md -->

<table>
<colgroup>
<col style="width: 13%" />
<col style="width: 31%" />
<col style="width: 16%" />
<col style="width: 8%" />
<col style="width: 29%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>address</td>
<td>Option
<Address></td>
<td>object</td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>boundary</td>
<td><code>Option&lt;MultiPolygon&lt;f64&gt;&gt;</code></td>
<td>geo_shape</td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>labels</td>
<td><code>I18nProperties</code></td>
<td>??</td>
<td>✓</td>
<td>used in dynamic templates</td>
</tr>
<tr class="even">
<td>names</td>
<td><code>I18nProperties</code></td>
<td></td>
<td>✓</td>
<td>used in dynamic templates</td>
</tr>
<tr class="odd">
<td>poi_type</td>
<td><code>PoiType</code></td>
<td>keyword</td>
<td>✓</td>
<td>used for filtering</td>
</tr>
<tr class="even">
<td>properties</td>
<td><code>BTreeMap&lt;String, String&gt;</code></td>
<td>object</td>
<td>✓</td>
<td>used for filtering</td>
</tr>
</tbody>
</table>

This leaves the remaining fields to be indexed with the
[mimir-poi.json](/config/elasticsearch/templates/indices/mimir-poi.json) index template.

#### <a id="stops-template"></a> Stops

If we look back at the [list of fields](#stops-fields) present in the stop document, and remove all
the fields that are part of the common template, we have the following list of remaining fields:

<!-- docs/assets/tbl/fields-2-stop.md -->

<table style="width:100%;">
<colgroup>
<col style="width: 23%" />
<col style="width: 33%" />
<col style="width: 20%" />
<col style="width: 10%" />
<col style="width: 12%" />
</colgroup>
<thead>
<tr class="header">
<th>field</th>
<th>type</th>
<th>Elasticsearch</th>
<th>Index</th>
<th>Comment</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>comments</td>
<td>Vec<Comment></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>commercial_modes</td>
<td>Vec<CommercialMode></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>coverages</td>
<td>Vec<String></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>feed_publishers</td>
<td>Vec<FeedPublisher></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>lines</td>
<td>Vec<Line></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="even">
<td>physical_modes</td>
<td>Vec<PhysicalMode></td>
<td></td>
<td>✗</td>
<td></td>
</tr>
<tr class="odd">
<td>properties</td>
<td>BTreeMap&lt;String, String&gt;</td>
<td>flattened</td>
<td>✓</td>
<td></td>
</tr>
<tr class="even">
<td>timezone</td>
<td>String</td>
<td></td>
<td>✗</td>
<td></td>
</tr>
</tbody>
</table>

This leaves the remaining fields to be indexed with the
[mimir-stop.json](/config/elasticsearch/templates/indices/mimir-stop.json) index template.

# Using Templates

## Importing Templates

For now there is a single binary that is used to insert templates in Elasticsearch. It must be
used prior to the creation of any index. This binary uses the same configuration / command line
configuration as the other binaries.

```
./target/release/ctlmimir -c ./config -m testing run
```

This program will look for the directories `<config>/ctlmimir`, and `<config>/elasticsearch` to read
some configuration values. and then scan `<config>/elasticsearch/templates/components` and import
all the templates in there, and same thing for `<config>/elasticsearch/templates/indices`.

You can check that all the templates directly in Elasticsearch: Since Mimirsbrunn's templates are
prefixed with 'mimir-', you can run:

```
curl -X GET 'http://localhost:9200/_component_template/mimir-*' | jq '.'
```

Same thing for index templates:

```
curl -X GET 'http://localhost:9200/_component_template/mimir-*' | jq '.'
```

## Overriding Templates

There are scenarios in which you may want to override certain values.

### For a certain type of index

Let's say you want to make sure that all administrative region indices have a certain number of
replicas, different from the default one. So, prior to importing the templates, you can change
the index template in `config/elasticsearch/templates/indices/mimir-admin.json` and change the
settings:

```json
{
  "elasticsearch": {
    "index_patterns": ["munin_admin*"],
    "template": {
      "settings": {
        "number_of_replicas": "2"
      }
      ...
    }
  }
}
```

Then, when you run ctlmimir, you will have a unique value for the number of replicas for all indices
starting with `munin_admin*`. You can then test that when you are creating a new index with
`cosmogony2mimir` you will have the correct number of replicas.

### For a certain index

Lets say that, following the previous scenario, you'd want to create a new admin index, but with 
a different number of replicas than that found in the index template.

In that case you can still use command line overrides:

```
cosmogony2mimir -s elasticsearch.settings.number_of_replicas=9 ...
```

# Updating Templates

Updating templates is essentially an iterative process, and we try to use a TDD approach:

* A new feature, a bug, and we create a new scenario in the *features* directory.
* We run the end to end tests (`cargo test --test end_to_end`), it fails
* We update the templates, and run the test again.

Playing with templates, analyzers, tokenizers, and so on, boosting some results with regards to
others requires an intimate knowledge of how 
## Evaluating Templates

These measures should be taken into account when modifying the templates: Like most iterative
process, we make a change, evaluate the results, estimate what needs to be changed to improve the
measure, and loop again.

Evaluating the templates can be done with:

* ctlmimir, which is a binary used to import the templates found in
  `/config/elasticsearchs/templates`. With this tool, we just check that we can actually import the
  templates.
* import2mimir.sh can be used to evaluate the whole indexing process, using ctl2mimir, and the other
  indexing tools.
* end to end tests are used to make sure that the indexing process is correct, and that searching 
  predefined queries results are correct.
* benchmark are used to estimate the time it takes to either index or search.


