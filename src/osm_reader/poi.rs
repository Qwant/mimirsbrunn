// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
//
// LICENCE: This program is free software; you can redistribute it
// and/or modify it under the terms of the GNU Affero General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public
// License along with this program. If not, see
// <http://www.gnu.org/licenses/>.
//
// Stay tuned using
// twitter @navitia
// IRC #navitia on freenode
// https://groups.google.com/d/forum/navitia
// www.navitia.io

use std::{collections::BTreeMap, io, ops::Deref, path::PathBuf};

use config::Config;
use mimir::domain::model::configuration::root_doctype;
use osm_boundaries_utils::build_boundary;
use places::{addr::Addr, street::Street};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use tracing::{info, instrument, warn};

use common::document::ContainerDocument;
use mimir::{
    adapters::primary::bragi::api::DEFAULT_LIMIT_RESULT_ES,
    domain::{model::query::Query, ports::primary::search_documents::SearchDocuments},
};
use places::{
    coord::Coord,
    i18n_properties::I18nProperties,
    poi::{Poi, PoiType},
};

use crate::{admin_geofinder::AdminGeoFinder, labels};

use super::{
    osm_utils::{get_way_coord, make_centroid},
    OsmPbfReader,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Obj Wrapper Error: {}", msg))]
    ObjWrapperCreation { msg: String },

    #[snafu(display("OsmPbfReader Extraction Error: {} [{}]", msg, source))]
    OsmPbfReaderExtraction {
        msg: String,
        source: osmpbfreader::Error,
    },

    #[snafu(display("JSON Deserialization Error [{}]", source))]
    JsonDeserialization { source: serde_json::Error },

    #[snafu(display("Poi Validation Error: {}", msg))]
    PoiValidation { msg: String },

    #[snafu(display("Config Merge Error: {} [{}]", msg, source))]
    ConfigMerge {
        msg: String,
        source: config::ConfigError,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsmTagsFilter {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rule {
    pub osm_tags_filters: Vec<OsmTagsFilter>,
    #[serde(rename = "type")]
    pub poi_type_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PoiConfig {
    #[serde(rename = "types")]
    pub poi_types: Vec<PoiType>,
    pub rules: Vec<Rule>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PoiWrapper {
    pub pois: crate::settings::osm2mimir::Poi,
}

impl Default for PoiConfig {
    fn default() -> Self {
        let base_path = env!("CARGO_MANIFEST_DIR");
        let input_dir: PathBuf = [base_path, "config", "osm2mimir"].iter().collect();
        let input_file = input_dir.join("default.toml");

        Config::default()
            .with_merged(config::File::from(input_file))
            .expect("cannot build the default poi configuration")
            .get("pois.config")
            .expect("poi configuration")
    }
}

impl PoiConfig {
    pub fn from_reader<R: io::Read>(r: R) -> Result<PoiConfig, Error> {
        let config: PoiConfig = serde_json::from_reader(r).context(JsonDeserializationSnafu)?;
        config.check()?;
        Ok(config)
    }
    pub fn is_poi(&self, tags: &osmpbfreader::Tags) -> bool {
        self.get_poi_type(tags).is_some()
    }
    pub fn get_poi_id(&self, tags: &osmpbfreader::Tags) -> Option<&str> {
        self.get_poi_type(tags).map(|poi_type| poi_type.id.as_str())
    }
    pub fn get_poi_type(&self, tags: &osmpbfreader::Tags) -> Option<&PoiType> {
        self.rules
            .iter()
            .find(|rule| {
                rule.osm_tags_filters
                    .iter()
                    .all(|f| tags.get(f.key.as_str()).map_or(false, |v| v == &f.value))
            })
            .and_then(|rule| {
                self.poi_types
                    .iter()
                    .find(|poi_type| poi_type.id == rule.poi_type_id)
            })
    }
    pub fn check(&self) -> Result<(), Error> {
        use std::collections::BTreeSet;
        let mut ids = BTreeSet::<&str>::new();
        for poi_type in &self.poi_types {
            if !ids.insert(&poi_type.id) {
                return Err(Error::PoiValidation {
                    msg: format!("poi_type_id {:?} present several times", poi_type.id),
                });
            }
        }
        for rule in &self.rules {
            if !ids.contains(rule.poi_type_id.as_str()) {
                return Err(Error::PoiValidation {
                    msg: format!("poi_type_id {:?} in a rule not declared", rule.poi_type_id),
                });
            }
        }
        Ok(())
    }
}

fn make_properties(tags: &osmpbfreader::Tags) -> BTreeMap<String, String> {
    tags.iter()
        .map(|(tag, value)| (tag.as_str().into(), value.as_str().into()))
        .collect()
}

fn parse_poi(
    osmobj: &osmpbfreader::OsmObj,
    obj_map: &BTreeMap<osmpbfreader::OsmId, osmpbfreader::OsmObj>,
    matcher: &PoiConfig,
    admins_geofinder: &AdminGeoFinder,
) -> Option<Poi> {
    let poi_type = match matcher.get_poi_type(osmobj.tags()) {
        Some(poi_type) => poi_type,
        None => {
            warn!(
                "The poi {:?} has no tags even if it passes the filters",
                osmobj.id()
            );
            return None;
        }
    };
    let (id, coord) = match *osmobj {
        osmpbfreader::OsmObj::Node(ref node) => (
            format_poi_id("node", node.id.0),
            Coord::new(node.lon(), node.lat()),
        ),
        osmpbfreader::OsmObj::Way(ref way) => {
            (format_poi_id("way", way.id.0), get_way_coord(obj_map, way))
        }
        osmpbfreader::OsmObj::Relation(ref relation) => (
            format_poi_id("relation", relation.id.0),
            make_centroid(&build_boundary(relation, obj_map)),
        ),
    };

    let name = osmobj
        .tags()
        .get("name")
        .map_or(poi_type.name.as_str(), |v| v.as_ref());

    if coord.is_default() {
        info!(
            "The poi {} is rejected, cause: could not compute coordinates.",
            id
        );
        return None;
    }

    let admins = admins_geofinder.get(&coord);
    let zip_codes = match osmobj.tags().get("addr:postcode") {
        Some(val) if !val.is_empty() => vec![val.to_string()],
        _ => places::admin::get_zip_codes_from_admins(&admins),
    };
    let country_codes = places::admin::find_country_codes(admins.iter().map(|a| a.deref()));
    Some(Poi {
        id,
        name: name.to_string(),
        label: labels::format_poi_label(name, admins.iter().map(|a| a.deref()), &country_codes),
        coord,
        approx_coord: Some(coord.into()),
        zip_codes,
        administrative_regions: admins,
        weight: 0.,
        poi_type: poi_type.clone(),
        properties: make_properties(osmobj.tags()),
        address: None,
        names: I18nProperties::default(),
        labels: I18nProperties::default(),
        distance: None,
        country_codes,
        context: None,
    })
}

fn format_poi_id(osm_type: &str, id: i64) -> String {
    format!("poi:osm:{}:{}", osm_type, id)
}

// FIXME Should produce a stream
#[instrument(skip(osm_reader, admins_geofinder))]
pub fn pois(
    osm_reader: &mut OsmPbfReader,
    matcher: &PoiConfig,
    admins_geofinder: &AdminGeoFinder,
) -> Result<Vec<Poi>, Error> {
    let objects = osm_reader
        .get_objs_and_deps(|o| matcher.is_poi(o.tags()))
        .context(OsmPbfReaderExtractionSnafu {
            msg: String::from("Could not read objects and dependencies from pbf"),
        })?;
    Ok(objects
        .iter()
        .filter(|&(_, obj)| matcher.is_poi(obj.tags()))
        .filter_map(|(_, obj)| parse_poi(obj, &objects, matcher, admins_geofinder))
        .collect())
}

pub fn compute_weight(poi: Poi) -> Poi {
    let weight = poi
        .administrative_regions
        .iter()
        .find(|admin| admin.is_city())
        .map(|admin| admin.weight);
    match weight {
        Some(weight) => Poi { weight, ..poi },
        None => poi,
    }
}

// FIXME Return a Result
pub async fn add_address<T>(backend: &T, poi: Poi, max_distance_reverse: usize) -> Poi
where
    T: SearchDocuments,
{
    let reverse = mimir::adapters::primary::common::dsl::build_reverse_query(
        format!("{}m", max_distance_reverse).as_ref(),
        poi.coord.lat(),
        poi.coord.lon(),
    );

    let es_indices_to_search = vec![
        root_doctype(Street::static_doc_type()),
        root_doctype(Addr::static_doc_type()),
    ];

    let documents = backend
        .search_documents(
            es_indices_to_search,
            Query::QueryDSL(reverse),
            DEFAULT_LIMIT_RESULT_ES,
            None,
        )
        .await;

    // FIXME ladder code, should use Result<(), Error> and combinators
    match documents {
        // Ok(res) => match serde_json::from_value(res) {
        Ok(addresses) => match addresses.into_iter().next() {
            Some(a) => Poi {
                address: Some(a),
                ..poi
            },
            None => {
                warn!(
                    "Cannot find a closest address for poi {:?} at lat {} / lon {}",
                    poi.id,
                    poi.coord.lat(),
                    poi.coord.lon()
                );
                poi
            }
        },
        Err(err) => {
            warn!(
                "Cannot deserialize reverse query for poi.id {:?}: {}",
                poi.id,
                err.to_string()
            );
            poi
        } // },
          // Err(err) => {
          //     warn!(
          //         "Cannot execute reverse query for poi {:?}: {}",
          //         poi.id,
          //         err.to_string()
          //     );
          // }
    }

    // poi.address = rubber
    //     .get_address(&poi.coord)
    //     .ok()
    //     .and_then(|addrs| addrs.into_iter().next())
    //     .map(|addr| addr.address().unwrap());
    // if poi.address.is_none() {
    //     warn!("The poi {:?} {:?} doesn't have address", poi.id, poi.name);
    // }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    fn tags(v: &[(&str, &str)]) -> osmpbfreader::Tags {
        v.iter().map(|&(k, v)| (k.into(), v.into())).collect()
    }

    fn from_str(s: &str) -> Result<PoiConfig, Error> {
        PoiConfig::from_reader(io::Cursor::new(s))
    }

    #[test]
    fn default_test() {
        let c = PoiConfig::default();
        assert!(c.get_poi_id(&tags(&[])).is_none());
        for s in &[
            "college",
            "university",
            "theatre",
            "hospital",
            "post_office",
            "bicycle_rental",
            "bicycle_parking",
            "parking",
            "police",
        ] {
            assert_eq!(
                format!("poi_type:amenity:{}", s),
                c.get_poi_id(&tags(&[("amenity", s)])).unwrap()
            );
        }
        for s in &["garden", "park"] {
            assert_eq!(
                format!("poi_type:leisure:{}", s),
                c.get_poi_id(&tags(&[("leisure", s)])).unwrap()
            );
        }
    }

    #[test]
    fn parsing_errors() {
        from_str("").unwrap_err();
        from_str("{}").unwrap_err();
        from_str("42").unwrap_err();
        from_str("{").unwrap_err();
        from_str(r#"{"types": []}"#).unwrap_err();
        from_str(r#"{"rules": []}"#).unwrap_err();
        from_str(r#"{"types": [], "rules": []}"#).unwrap();
        from_str(r#"{"types": [{"id": "poi_type:foo"}], "rules": []}"#).unwrap_err();
        from_str(r#"{"types": [{"name": "bar"}], "rules": []}"#).unwrap_err();
        from_str(r#"{"types": [{"id": "poi_type:foo", "name": "bar"}], "rules": []}"#).unwrap();
    }

    #[test]
    fn check_tests() {
        from_str(
            r#"{
            "types": [
                {"id": "poi_type:bob", "name": "Bob"},
                {"id": "poi_type:bob", "name": "Bobitto"}
            ],
            "rules": []
        }"#,
        )
        .unwrap_err();
        from_str(
            r#"{
            "types": [{"id": "poi_type:bob", "name": "Bob"}],
            "rules": [
                {
                    "osm_tags_filters": [{"key": "foo", "value": "bar"}],
                    "type": "bobette"
                }
            ]
        }"#,
        )
        .unwrap_err();
    }

    #[test]
    fn check_with_colon() {
        let json = r#"{
            "types": [
                {"id": "poi_type:amenity:bicycle_rental", "name": "Station VLS"},
                {"id": "poi_type:amenity:parking", "name": "Parking"}
            ],
            "rules": [
                {
                    "osm_tags_filters": [
                        {"key": "bicycle_rental", "value": "true"}
                    ],
                    "type": "poi_type:amenity:bicycle_rental"
                },
                {
                    "osm_tags_filters": [
                        {"key": "amenity", "value": "parking:effia"}
                    ],
                    "type": "poi_type:amenity:parking"
                }
            ]
        }"#;
        let c = from_str(json).unwrap();
        assert_eq!(
            Some("poi_type:amenity:bicycle_rental"),
            c.get_poi_id(&tags(&[("bicycle_rental", "true")]))
        );
        assert_eq!(
            Some("poi_type:amenity:parking"),
            c.get_poi_id(&tags(&[("amenity", "parking:effia")]))
        );
    }

    #[test]
    fn check_all_tags_first_match() {
        let json = r#"{
            "types": [
                {"id": "poi_type:bob_titi", "name": "Bob is Bobette and Titi is Toto"},
                {"id": "poi_type:bob", "name": "Bob is Bobette"},
                {"id": "poi_type:titi", "name": "Titi is Toto"},
                {"id": "poi_type:foo", "name": "Foo is Bar"}
            ],
            "rules": [
                {
                    "osm_tags_filters": [
                        {"key": "bob", "value": "bobette"},
                        {"key": "titi", "value": "toto"}
                    ],
                    "type": "poi_type:bob_titi"
                },
                {
                    "osm_tags_filters": [
                        {"key": "bob", "value": "bobette"}
                    ],
                    "type": "poi_type:bob"
                },
                {
                    "osm_tags_filters": [
                        {"key": "titi", "value": "toto"}
                    ],
                    "type": "poi_type:titi"
                },
                {
                    "osm_tags_filters": [
                        {"key": "foo", "value": "bar"}
                    ],
                    "type": "poi_type:foo"
                }
            ]
        }"#;
        let c = from_str(json).unwrap();
        assert_eq!(
            Some("poi_type:bob"),
            c.get_poi_id(&tags(&[
                ("bob", "bobette"),
                ("titi", "tata"),
                ("foo", "bar"),
            ],))
        );
        assert_eq!(
            Some("poi_type:titi"),
            c.get_poi_id(&tags(&[
                ("bob", "bobitta"),
                ("titi", "toto"),
                ("foo", "bar"),
            ],))
        );
        assert_eq!(
            Some("poi_type:bob_titi"),
            c.get_poi_id(&tags(&[
                ("bob", "bobette"),
                ("titi", "toto"),
                ("foo", "bar"),
            ],))
        );
        assert_eq!(
            Some("poi_type:foo"),
            c.get_poi_id(&tags(&[
                ("bob", "bobitta"),
                ("titi", "tata"),
                ("foo", "bar"),
            ],))
        );
    }
}
