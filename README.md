[![GitHub license](https://img.shields.io/github/license/qwant/mimirsbrunn.svg)](https://github.com/qwant/mimirsbrunn/blob/master/LICENSE)
[![GitHub tag](https://img.shields.io/github/tag/qwant/mimirsbrunn.svg)](https://github.com/qwant/mimirsbrunn/tag)

# Mimirsbrunn

⚠️ **We are making a lot of changes to mimir and the current documentation is outdated** ⚠️

Mimirsbrunn (also called Mimir) is an independent geocoding and reverse-geocoding system written in
[Rust](https://www.rust-lang.org/en-US/), and built upon [Elasticsearch](https://www.elastic.co). It
can handle addresses, streets, points-of-interest (POI), administrative regions and public transport
stops.

## What's a Geocoder ?

Usually [geocoding](https://en.wikipedia.org/wiki/Geocoding) refers to "the process of transforming
a physical address description to a location on the Earth's surface". However Mimir is more a
[geoparser](https://en.wikipedia.org/wiki/Toponym_resolution#Geoparsing) than a geocoder since it
can resolve any ambiguous toponym to its correct location.

In other words, a geocoder reads a description (possibly incomplete) of a location, and returns a
list of candidate locations (latitude / longitude) matching the input. 

Geocoding is traditionally used for autocompleting search fields used in geographic applications.
For example, here is a screenshot of Qwant Maps, where the user enters a search string `20 rue hec
mal`, and mimir returns possible candidates in a dropdown box.

## License

This project is licensed under the [AGPLv3](LICENSE.md) GNU Affero General Public License - see the
[LICENSE.md](LICENSE.md) file for details



