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
use itertools::Itertools;
use std::collections::BTreeSet;

pub type StreetsVec = Vec<places::street::Street>;

#[derive(Debug)]
pub struct AdminMatcher {
    admin_levels: BTreeSet<u32>,
}

impl AdminMatcher {
    pub fn new(levels: BTreeSet<u32>) -> AdminMatcher {
        AdminMatcher {
            admin_levels: levels,
        }
    }

    pub fn is_admin(&self, obj: &osmpbfreader::OsmObj) -> bool {
        match *obj {
            osmpbfreader::OsmObj::Relation(ref rel) => {
                rel.tags
                    .get("boundary")
                    .map_or(false, |v| v == "administrative")
                    && rel.tags.get("admin_level").map_or(false, |lvl| {
                        self.admin_levels.contains(&lvl.parse::<u32>().unwrap_or(0))
                    })
            }
            _ => false,
        }
    }
}

pub fn format_zip_codes(zip_codes: &[String]) -> String {
    match zip_codes.len() {
        0 => "".to_string(),
        1 => format!(" ({})", zip_codes.first().unwrap()),
        _ => format!(
            " ({}-{})",
            zip_codes.first().unwrap(),
            zip_codes.last().unwrap()
        ),
    }
}

pub fn read_zip_codes(tags: &osmpbfreader::Tags) -> Vec<String> {
    let zip_code = tags
        .get("addr:postcode")
        .or_else(|| tags.get("postal_code"))
        .map_or("", |val| &val[..]);
    zip_code
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .sorted()
        .collect()
}

pub fn read_insee(tags: &osmpbfreader::Tags) -> Option<&str> {
    tags.get("ref:INSEE").map(|v| v.as_str())
}
