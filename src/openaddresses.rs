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

use crate::error::Error;
use places::addr::Addr;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

use crate::{admin_geofinder::AdminGeoFinder, labels};
use places::admin::find_country_codes;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct OpenAddress {
    pub id: String,
    pub street: String,
    pub postcode: String,
    pub district: String,
    pub region: String,
    pub city: String,
    pub number: String,
    pub unit: String,
    pub lat: f64,
    pub lon: f64,
}

impl OpenAddress {
    pub fn into_addr(
        self,
        admins_geofinder: &AdminGeoFinder,
        id_precision: usize,
    ) -> Result<Addr, Error> {
        let street_id = format!("street:{}", self.id); // TODO check if thats ok
        let admins = admins_geofinder.get(&geo::Coordinate {
            x: self.lon,
            y: self.lat,
        });
        let country_codes = find_country_codes(admins.iter().map(|a| a.deref()));

        let weight = admins.iter().find(|a| a.is_city()).map_or(0., |a| a.weight);
        // Note: for openaddress, we don't trust the admin hierarchy much (compared to bano)
        // so we use for the label the admins that we find in the DB
        let street_label = labels::format_street_label(
            &self.street,
            admins.iter().map(|a| a.deref()),
            &country_codes,
        );
        let (addr_name, addr_label) = labels::format_addr_name_and_label(
            &self.number,
            &self.street,
            admins.iter().map(|a| a.deref()),
            &country_codes,
        );

        let zip_codes: Vec<_> = self.postcode.split(';').map(str::to_string).collect();
        let coord = places::coord::Coord::new(self.lon, self.lat);
        let street = places::street::Street {
            id: street_id,
            name: self.street,
            label: street_label,
            administrative_regions: admins,
            weight,
            zip_codes: zip_codes.clone(),
            coord,
            approx_coord: None,
            distance: None,
            country_codes: country_codes.clone(),
            context: None,
        };

        let id_suffix = (self.number)
            .replace([' ', '\t', '\r', '\n'], "")
            .replace(['/', '.', ':', ';'], "-");

        let id = {
            if id_precision > 0 {
                format!(
                    "addr:{:.precision$};{:.precision$}:{}",
                    self.lon,
                    self.lat,
                    id_suffix,
                    precision = id_precision
                )
            } else {
                format!("addr:{};{}:{}", self.lon, self.lat, id_suffix)
            }
        };

        Ok(Addr {
            id,
            name: addr_name,
            label: addr_label,
            house_number: self.number,
            street,
            coord,
            approx_coord: Some(coord.into()),
            weight,
            zip_codes,
            distance: None,
            country_codes,
            context: None,
        })
    }
}
