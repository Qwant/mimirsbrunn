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

use super::osm_store::Getter;
use geo::{centroid::Centroid, MultiPolygon};
use std::collections::BTreeMap;

pub fn get_way_coord<T: Getter>(
    obj_map: &T,
    way: &osmpbfreader::objects::Way,
) -> Result<places::coord::Coord, places::coord::CoordError> {
    /*
        Returns arbitrary Coord on the way.
        A middle node is chosen as a better marker on a street
        than the first node.
    */
    let nb_nodes = way.nodes.len();
    way.nodes
        .iter()
        .skip(nb_nodes / 2)
        .find_map(|node_id| {
            obj_map
                .get(&(*node_id).into())?
                .node()
                .map(|node| places::coord::Coord::new(node.lon(), node.lat()))
        })
        .unwrap_or_else(|| Ok(places::coord::Coord::default()))
}

pub fn make_centroid(
    boundary: &Option<MultiPolygon<f64>>,
) -> Result<places::coord::Coord, places::coord::CoordError> {
    boundary
        .as_ref()
        .and_then(|b| {
            b.centroid()
                .map(|c| places::coord::Coord::new(c.x(), c.y()))
        })
        .unwrap_or_else(|| Ok(places::coord::Coord::default()))
}

pub fn get_osm_codes_from_tags(tags: &osmpbfreader::Tags) -> BTreeMap<String, String> {
    // read codes from osm tags
    // for the moment we only use:
    // * ISO3166 codes (mainly to get country codes)
    // * ref:* tags (to get NUTS codes, INSEE code (even if we have a custom field for them), ...)
    tags.iter()
        .filter(|(k, _)| k.starts_with("ISO3166") || k.starts_with("ref:") || *k == "wikidata")
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

pub fn get_label_languages_from_tags(
    tags: &osmpbfreader::Tags,
    label: &str,
    langs: &[String],
) -> places::i18n_properties::I18nProperties {
    let properties = tags
        .iter()
        .filter(|(k, _)| k.starts_with(&label))
        .map(|property| places::Property {
            key: property.0[label.len()..].to_string(),
            value: property.1.to_string(),
        })
        .filter(|p| langs.contains(&p.key))
        .collect();
    places::i18n_properties::I18nProperties(properties)
}
