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

use crate::{
    admin_geofinder::AdminGeoFinder,
    error::{Error, InvalidFantoirIdSnafu, InvalidInseeIdSnafu},
    labels,
};
use places::{addr::Addr, admin::Admin, coord::Coord, street::Street};
use serde::{Deserialize, Serialize};
use snafu::ensure;
use std::{collections::BTreeMap, ops::Deref, sync::Arc};

type AdminFromInsee = BTreeMap<String, Arc<Admin>>;

#[derive(Serialize, Deserialize)]
pub struct Bano {
    pub id: String,
    pub house_number: String,
    pub street: String,
    pub zip: String,
    pub city: String,
    pub src: String,
    pub lat: f64,
    pub lon: f64,
}

impl Bano {
    pub fn insee(&self) -> Result<&str, Error> {
        ensure!(self.id.len() >= 5, InvalidInseeIdSnafu { id: &self.id });
        Ok(self.id[..5].trim_start_matches('0'))
    }
    pub fn fantoir(&self) -> Result<&str, Error> {
        ensure!(self.id.len() >= 10, InvalidFantoirIdSnafu { id: &self.id });
        Ok(&self.id[..10])
    }
    pub fn into_addr(
        self,
        admins_from_insee: &AdminFromInsee,
        admins_geofinder: &AdminGeoFinder,
    ) -> Result<Addr, Error> {
        let street_id = format!("street:{}", self.fantoir()?);
        let mut admins = admins_geofinder.get(&geo::Coordinate {
            x: self.lon,
            y: self.lat,
        });

        // If we have an admin corresponding to the INSEE, we know
        // that's the good one, thus we remove all the admins of its
        // level found by the geofinder, and add our admin.
        if let Some(admin) = admins_from_insee.get(self.insee()?) {
            admins.retain(|a| a.level != admin.level);
            admins.push(admin.clone());
        }

        let country_codes = vec!["fr".to_owned()];

        // to format the label of the addr/street, we use bano's city
        // even if we already have found a city in the admin_geo_finder
        let city = build_admin_from_bano_city(&self.city);
        let zones_for_label_formatting = admins
            .iter()
            .filter(|a| a.is_city())
            .map(|a| a.deref())
            .chain(std::iter::once(&city));

        let street_label = labels::format_street_label(
            &self.street,
            zones_for_label_formatting.clone(),
            &country_codes,
        );
        let (addr_name, addr_label) = labels::format_addr_name_and_label(
            &self.house_number,
            &self.street,
            zones_for_label_formatting,
            &country_codes,
        );

        let weight = admins
            .iter()
            .find(|a| a.level == 8)
            .map_or(0., |a| a.weight);

        let zip_codes: Vec<_> = self.zip.split(';').map(str::to_string).collect();
        let coord = Coord::new(self.lon, self.lat);
        let street = Street {
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
        Ok(Addr {
            id: format!(
                "addr:{};{}:{}",
                self.lon,
                self.lat,
                self.house_number
                    .replace([' ', '\t', '\r', '\n'], "")
                    .replace(['/', '.', ':', ';'], "-")
            ),
            name: addr_name,
            label: addr_label,
            house_number: self.house_number,
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

fn build_admin_from_bano_city(city: &str) -> Admin {
    Admin {
        name: city.to_string(),
        zone_type: Some(cosmogony::ZoneType::City),
        ..Default::default()
    }
}
