# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is generated automatically by the release procedure, please do not edit.

---

## v1.18.1

Released 2020-12-03

### Upgraded

* Upgrade dependencies on geo, osmpbfreafer, osm_boundaries_utils, ... (Adrien Matissart, [441](https://github.com/hove-io/mimirsbrunn/pull/441))

---

## v1.18.0

Released 2020-12-03

## Added

* We can accept multiple zipcode in BANO input  (Rémi Dupré, [442](https://github.com/hove-io/mimirsbrunn/pull/442))
* osm2mimir uses a configuration file (Matthieu Paindavoine, [444](https://github.com/hove-io/mimirsbrunn/pull/444))
* Some documentation (Matthieu Paindavoine, [437](https://github.com/hove-io/mimirsbrunn/pull/437))

## Fixed

* Github Action generation of debian packages (Pascas Benchimol, [443](https://github.com/hove-io/mimirsbrunn/pull/443))

## Changed

* Generation of docker images uses debian and rust arguments (Matthieu Paindavoine, [439](https://github.com/hove-io/mimirsbrunn/pull/439))
* Parameter for maximum bulk insertion error (default 0) (Adrien Matissart, [440](https://github.com/hove-io/mimirsbrunn/pull/440))
* Elision of some French letters (d', l') (Rémi Dupré, [430](https://github.com/hove-io/mimirsbrunn/pull/430))
* Better handling of streets at admin's borders (Rémi Dupré, [424](https://github.com/hove-io/mimirsbrunn/pull/424))
