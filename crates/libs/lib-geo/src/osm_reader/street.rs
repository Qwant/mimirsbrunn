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

use std::fmt::Write;
use std::ops::Deref;
use std::sync::Arc;

use cosmogony::ZoneType;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::admin_geofinder::AdminGeoFinder;
use crate::labels;
use crate::osm_reader::errors::OsmReaderError;
use crate::utils::slice::for_each_group;

use super::osm_store::{Getter, ObjWrapper};
use super::osm_utils::get_way_coord;
use super::OsmPbfReader;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum Kind {
    Node = 0,
    Way = 1,
    Relation = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreetExclusion {
    pub highway: Option<Vec<String>>,
    pub public_transport: Option<Vec<String>>,
}

/// Get the city ID given street belongs in, if there is none give the smallest admin.
fn get_street_city_or_relation(street: &places::street::Street) -> Option<&str> {
    street
        .administrative_regions
        .iter()
        .find(|admin| admin.is_city())
        .or_else(|| street.administrative_regions.first())
        .map(|admin| admin.id.as_str())
}

/// Deduplicate streets by name and city in place. If two objects have the same city and street
/// name, they are duplicated and store with a different id
fn deduplicate_streets(street_list: &mut Vec<places::street::Street>) {
    let get_street_dedup_key = |street: &places::street::Street| {
        let name = street.name.as_str().to_string();

        let city = get_street_city_or_relation(street)
            .unwrap_or("")
            .to_string();

        (name, city)
    };

    // Keeping a stable sort ensures that streets that were added first will be kept in priority.
    street_list.sort_by_key(get_street_dedup_key);
    street_list.dedup_by_key(|s| get_street_dedup_key(s));
    street_list.shrink_to_fit();
}

/// Ensure all streets have a unique ID by adding a number to all streets which conflict with
/// another.
fn ensure_unique_street_id(street_list: &mut [places::street::Street]) {
    // Streets will be ordered by ID to detect duplicates and then by admins to have a stable ID.
    let get_street_sort_key = |street: &places::street::Street| {
        let id = street.id.to_string();

        let city = get_street_city_or_relation(street)
            .unwrap_or("")
            .to_string();

        (id, city)
    };

    street_list.sort_unstable_by_key(get_street_sort_key);

    for_each_group(
        street_list,
        |street| street.id.clone(),
        |group| {
            if group.len() > 1 {
                for (i, street) in group.iter_mut().enumerate() {
                    write!(street.id, "-{i}").unwrap();
                }
            }
        },
    );
}

// The following conditional compilation is to allow to optionaly pass an extra argument if
// the db-storage feature is enabled
#[cfg(feature = "db-storage")]
pub fn streets(
    osm_reader: &mut OsmPbfReader,
    admins_geofinder: &AdminGeoFinder,
    exclusions: &StreetExclusion,
    database: Option<&super::database::Database>,
) -> Result<Vec<places::street::Street>, OsmReaderError> {
    let objs_map = ObjWrapper::new(database)?;
    inner_streets(osm_reader, admins_geofinder, exclusions, objs_map)
}

#[cfg(not(feature = "db-storage"))]
pub fn streets(
    osm_reader: &mut OsmPbfReader,
    admins_geofinder: &AdminGeoFinder,
    exclusions: &StreetExclusion,
) -> Result<Vec<places::street::Street>, OsmReaderError> {
    let objs_map = ObjWrapper::new()?;
    inner_streets(osm_reader, admins_geofinder, exclusions, objs_map)
}

#[instrument(skip(osm_reader, admins_geofinder, objs_map))]
pub fn inner_streets(
    osm_reader: &mut OsmPbfReader,
    admins_geofinder: &AdminGeoFinder,
    exclusions: &StreetExclusion,
    mut objs_map: ObjWrapper,
) -> Result<Vec<places::street::Street>, OsmReaderError> {
    let invalid_highways = exclusions.highway.as_deref().unwrap_or(&[]);

    let is_valid_highway = |tag: &str| -> bool { !invalid_highways.iter().any(|k| k == tag) };

    let invalid_public_transports = exclusions.public_transport.as_deref().unwrap_or(&[]);

    let is_valid_public_transport =
        |tag: &str| -> bool { !invalid_public_transports.iter().any(|k| k == tag) };

    // For the object to be a valid street, it needs to be an osm highway of a valid type,
    // or a relation of type associatedStreet.
    let is_valid_obj = |obj: &osmpbfreader::OsmObj| -> bool {
        match *obj {
            osmpbfreader::OsmObj::Way(ref way) => {
                let has_valid_highway_tag = way
                    .tags
                    .get("highway")
                    .map_or(false, |v| !v.is_empty() && is_valid_highway(v));
                let has_no_excluded_public_transport_tag = way
                    .tags
                    .get("public_transport")
                    .map_or(true, |v| is_valid_public_transport(v));
                let has_valid_name_tag = way.tags.get("name").map_or(false, |v| !v.is_empty());
                has_valid_name_tag && has_valid_highway_tag && has_no_excluded_public_transport_tag
            }
            osmpbfreader::OsmObj::Relation(ref rel) => rel
                .tags
                .get("type")
                .map_or(false, |v| v == "associatedStreet"),
            _ => false,
        }
    };

    info!("reading pbf...");

    {
        #[cfg(feature = "db-storage")]
        let mut objs_map = objs_map.get_writter()?;

        osm_reader.get_objs_and_deps_store(is_valid_obj, &mut objs_map)?;
    }

    info!("reading pbf done");

    // Builder for street object
    let build_street = |id: String,
                        name: String,
                        alt_name: Option<String>,
                        loc_name: Option<String>,
                        old_name: Option<String>,
                        coord: places::coord::Coord,
                        admins: Vec<Arc<places::admin::Admin>>| {
        let admins_iter = admins.iter().map(Deref::deref);
        let country_codes = places::admin::find_country_codes(admins_iter.clone());
        places::street::Street {
            id,
            label: labels::format_street_label(&name, admins_iter, &country_codes),
            name,
            alt_name,
            loc_name,
            old_name,
            weight: 0.,
            zip_codes: places::admin::get_zip_codes_from_admins(&admins),
            administrative_regions: admins,
            coord,
            approx_coord: Some(coord.into()),
            distance: None,
            country_codes,
            context: None,
        }
    };

    // List of streets to output. There may be duplicates that will be removed before returning.
    let mut street_list = Vec::new();

    #[cfg(feature = "db-storage")]
    let objs_map = objs_map.get_reader()?;

    objs_map.for_each_filter(Kind::Relation, |obj| {
        let rel = obj.relation().expect("invalid relation filter");
        let rel_name = rel.tags.get("name");
        let rel_alt_name = rel.tags.get("alt_name");
        let rel_loc_name = rel.tags.get("loc_name");
        let rel_old_name = rel.tags.get("old_name");

        let rel_streets = rel
            .refs
            .iter()
            .filter(|ref_obj| ref_obj.member.is_way() && &ref_obj.role == "street")
            .find_map(|ref_obj| {
                let obj = objs_map.get(&ref_obj.member)?;
                let way = obj.way()?;
                let coord = get_way_coord(&objs_map, way).unwrap_or_default();
                let name = rel_name.or_else(|| way.tags.get("name"))?.to_string();
                let alt_name = rel_alt_name
                    .or_else(|| way.tags.get("alt_name"))
                    .map(|s| s.to_string());
                let loc_name = rel_loc_name
                    .or_else(|| way.tags.get("loc_name"))
                    .map(|s| s.to_string());
                let old_name = rel_old_name
                    .or_else(|| way.tags.get("old_name"))
                    .map(|s| s.to_string());

                Some(
                    get_street_admin(admins_geofinder, &objs_map, way)
                        .into_iter()
                        .map(move |admins| {
                            build_street(
                                format!("street:osm:relation:{}", rel.id.0),
                                name.to_string(),
                                alt_name.clone(),
                                loc_name.clone(),
                                old_name.clone(),
                                coord,
                                admins,
                            )
                        }),
                )
            })
            .into_iter()
            .flatten();

        street_list.extend(rel_streets);
    });

    let count_from_rels = street_list.len();
    info!("added {count_from_rels} streets from relations");

    objs_map.for_each_filter(Kind::Way, |obj| {
        let way = obj.way().expect("invalid way filter");

        if let Some(name) = way.tags.get("name") {
            let coords = get_way_coord(&objs_map, way).unwrap_or_default();
            let alt_name = way.tags.get("alt_name").map(|s| s.to_string());
            let loc_name = way.tags.get("loc_name").map(|s| s.to_string());
            let old_name = way.tags.get("old_name").map(|s| s.to_string());
            for admins in get_street_admin(admins_geofinder, &objs_map, way) {
                street_list.push(build_street(
                    format!("street:osm:way:{}", way.id.0),
                    name.to_string(),
                    alt_name.clone(),
                    loc_name.clone(),
                    old_name.clone(),
                    coords,
                    admins,
                ));
            }
        }
    });

    let count_from_ways = street_list.len() - count_from_rels;
    info!("added {count_from_ways} streets from ways");

    // Some streets can be indexed from several objects, either because of "bad" mapping or because
    // several sections of the road don't share the same characteristics.
    deduplicate_streets(&mut street_list);

    // A road represented by a single object can be added twice or more if it crosses admin
    // boundaries, we need to ensure they don't share the same ID.
    ensure_unique_street_id(&mut street_list);

    info!("finished deduplicating streets, {} left", street_list.len());
    Ok(street_list)
}

/// Returns branches of admins encompassing the street `way`.
fn get_street_admin<T: Getter>(
    admins_geofinder: &AdminGeoFinder,
    obj_map: &T,
    way: &osmpbfreader::objects::Way,
) -> Vec<Vec<Arc<places::admin::Admin>>> {
    let nb_nodes = way.nodes.len();

    // To avoid corner cases where the ends of the way are near
    // administrative boundaries, the geofinder is called
    // preferably on a middle node.
    let (nodes_left, nodes_right) = (
        way.nodes[..nb_nodes / 2].iter(),
        way.nodes[nb_nodes / 2..].iter(),
    );

    nodes_right
        .chain(nodes_left)
        .filter_map(|node_id| obj_map.get(&(*node_id).into()))
        .find_map(|node_obj| {
            node_obj.node().map(|node| geo_types::Coord {
                x: node.lon(),
                y: node.lat(),
            })
        })
        .map_or_else(Vec::new, |coord| {
            // If the coords are part of several cities or suburbs, they are
            // all part of the output together with their parents. For
            // performance reasons, if the admin hierarchy is built of zones
            // bigger than cities, at most one result will belong to the output.
            admins_geofinder.get_admins_if(&coord, |admin| {
                admin
                    .zone_type
                    .map(|zt| zt <= ZoneType::City)
                    .unwrap_or(false)
            })
        })
}
