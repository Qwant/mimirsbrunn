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

/// In this module we put the code related to stops, that need to draw on 'places', 'mimir',
/// 'common', and 'config' (ie all the workspaces that make up mimirsbrunn).
use futures::stream::{Stream, TryStreamExt};
use mimir::domain::model::configuration::{ContainerConfig, PhysicalModeWeight};
use snafu::{ResultExt, Snafu};
use std::{collections::HashMap, ops::Deref, path::Path, sync::Arc};
use tracing::{info, warn};

use crate::{
    admin::read_admin_in_cosmogony_file,
    admin_geofinder::AdminGeoFinder,
    labels,
    settings::{admin_settings::AdminSettings, ntfs2mimir::Settings},
};
use mimir::{
    adapters::secondary::elasticsearch::{self, ElasticsearchStorage},
    domain::ports::primary::{generate_index::GenerateIndex, list_documents::ListDocuments},
};
use places::{admin::Admin, stop::Stop};

#[derive(Debug, Snafu)]
pub enum Error {
    // #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    // Settings { source: settings::Error },
    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchPool {
        source: elasticsearch::remote::Error,
    },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: mimir::domain::ports::secondary::remote::Error,
    },

    // #[snafu(display("Cosmogony Error: {}", details))]
    // Cosmogony { details: String },
    #[snafu(display("Index Generation Error {}", source))]
    IndexGeneration {
        source: mimir::domain::model::error::Error,
    },

    // transit_model uses failure::Error, which does not implement std::Error, so
    // we use a String to get the error message instead.
    #[snafu(display("Transit Model Error {}", details))]
    TransitModel { details: String },

    #[snafu(display("Admin Retrieval Error {}", details))]
    AdminRetrieval { details: String },
}

pub fn initialize_weights<'a, It, S: ::std::hash::BuildHasher>(
    stops: It,
    nb_stop_points: &HashMap<String, u32, S>,
) where
    It: Iterator<Item = &'a mut Stop>,
{
    let max = *nb_stop_points.values().max().unwrap_or(&1) as f64;
    for stop in stops {
        stop.weight = if let Some(weight) = nb_stop_points.get(&stop.id) {
            *weight as f64 / max
        } else {
            0.0
        };
    }
}

pub fn build_stop_area_weight(
    navitia: &transit_model::Model,
    physical_mode_weight: &Option<Vec<PhysicalModeWeight>>,
) -> HashMap<String, f64> {
    let md_weight_hash_map: HashMap<String, f64> = match physical_mode_weight {
        Some(modes) => modes
            .iter()
            .map(|mode| (mode.id.to_string().to_lowercase(), mode.weight as f64))
            .collect::<HashMap<String, f64>>(),
        _ => HashMap::new(),
    };
    let mut result = HashMap::new();
    for (idx, sp) in &navitia.stop_points {
        let stop_area = places::utils::normalize_id("stop_area", &sp.stop_area_id);

        let mut weights: f64 = navitia
            .get_corresponding_from_idx::<_, transit_model::objects::PhysicalMode>(idx)
            .into_iter()
            .map(|idx| {
                let ph_mode = &navitia.physical_modes[idx].id;
                let pm_w = md_weight_hash_map.get(&ph_mode.to_lowercase());
                match pm_w {
                    Some(value) => *value,
                    _ => {
                        warn!("Physical mode, id: {}, not found in mimir config.", ph_mode);
                        0.0
                    }
                }
            })
            .filter(|&weight| weight != 0.0)
            .into_iter()
            .sum();
        let res = result.get(&stop_area);
        if let Some(value) = res {
            weights += value;
        }
        result.insert(stop_area, weights);
    }
    result
}

pub fn make_weight(stop: &mut Stop, stop_areas_weights: &HashMap<String, f64>) {
    // Admin weight
    let admin_weight = stop
        .administrative_regions
        .iter()
        .filter(|adm| adm.is_city())
        .map(|adm| adm.weight)
        .next()
        .unwrap_or(0.0);
    // FIXME: 1024, automagic!
    // It's a factor used to bring the stop weight and the admin weight in the same order of
    // magnitude...
    // We then use a log to compress the distance between low admin weight and high ones.
    // admin_weight = admin_weight * 1024.0 + 1.0;
    // admin_weight = admin_weight.log10();

    let weights = stop_areas_weights.get(&*stop.id);
    if let Some(value) = weights {
        stop.weight = (value + admin_weight) / (2_f64);
    } else {
        stop.weight = admin_weight
    }
}

fn attach_stop(stop: &mut Stop, admins: Vec<Arc<Admin>>) {
    let admins_iter = admins.iter().map(|a| a.deref());
    let country_codes = places::admin::find_country_codes(admins_iter.clone());

    stop.label = labels::format_stop_label(&stop.name, admins_iter, &country_codes);
    stop.zip_codes = places::admin::get_zip_codes_from_admins(&admins);

    stop.country_codes = country_codes;
    stop.administrative_regions = admins;
}

async fn attach_stops_to_admin<'s, Stops>(
    stops: Stops,
    admin_settings: &AdminSettings,
    client: &ElasticsearchStorage,
) -> Result<(), Error>
where
    Stops: Iterator<Item = &'s mut Stop>,
{
    match admin_settings {
        AdminSettings::Elasticsearch => attach_stops_to_admins_from_es(stops, client).await,
        AdminSettings::Local(local_config) => {
            let admins = read_admin_in_cosmogony_file(local_config).map_err(|err| {
                Error::AdminRetrieval {
                    details: err.to_string(),
                }
            })?;
            attach_stops_to_admins_from_iter(stops, admins);
            Ok(())
        }
    }
}

/// Attach the stops to administrative regions
///
/// The admins are loaded from Elasticsearch and stored in a quadtree
/// We attach a stop with all the admins that have a boundary containing
/// the coordinate of the stop
async fn attach_stops_to_admins_from_es<'a, It: Iterator<Item = &'a mut Stop>>(
    stops: It,
    client: &ElasticsearchStorage,
) -> Result<(), Error> {
    match client.list_documents().await {
        Ok(stream) => {
            let admins: Vec<Admin> = stream.try_collect().await.context(IndexGenerationSnafu)?;

            if admins.is_empty() {
                return Err(Error::AdminRetrieval {
                    details: String::from("no admin retrieved to enrich stops"),
                });
            }
            info!("{} admins retrieved from ES ", admins.len());
            attach_stops_to_admins_from_iter(stops, admins.into_iter());
            Ok(())
        }
        Err(_) => Err(Error::AdminRetrieval {
            details: String::from("Could not retrieve admins to enrich stops"),
        }),
    }
}

/// Attach the stops to administrative regions
///
/// The admins are stored in a quadtree
/// We attach a stop with all the admins that have a boundary containing
/// the coordinate of the stop
fn attach_stops_to_admins_from_iter<'stop, Stops, Admins>(stops: Stops, admins: Admins)
where
    Stops: Iterator<Item = &'stop mut Stop>,
    Admins: Iterator<Item = Admin>,
{
    let mut nb_unmatched = 0;
    let mut nb_matched = 0;
    let admins_geofinder = admins.collect::<AdminGeoFinder>();
    for stop in stops {
        let admins = admins_geofinder.get(&stop.coord);

        if admins.is_empty() {
            nb_unmatched += 1;
        } else {
            nb_matched += 1;
        }

        attach_stop(stop, admins);
    }

    info!(
        "there are {}/{} stops without any admin",
        nb_unmatched,
        nb_matched + nb_unmatched
    );
}

/// Stores the stops found in the 'input' directory, in Elasticsearch, with the given
/// configuration.
///
/// The main part of this function is to actually create a list of stops
/// from the information found in the NTFS directory.
pub async fn index_ntfs(
    input: &Path,
    settings: &Settings,
    client: &ElasticsearchStorage,
) -> Result<(), Error> {
    let mut stops = {
        let navitia = transit_model::ntfs::read(&input).map_err(|err| Error::TransitModel {
            details: format!(
                "Could not read transit model from {}: {}",
                input.display(),
                err
            ),
        })?;

        info!("Build stops weight by physical modes");
        let stop_areas_weights = build_stop_area_weight(&navitia, &settings.physical_mode_weight);

        info!("Make mimir stops from navitia stops");
        let mut stops: Vec<Stop> = navitia
            .stop_areas
            .iter()
            .map(|(idx, sa)| places::stop::to_mimir(idx, sa, &navitia))
            .collect();

        info!("Make weights for stops");
        for stop in &mut stops {
            stop.coverages.push(settings.container.dataset.clone());
            make_weight(stop, &stop_areas_weights);
        }

        stops
    };

    info!("Attach stops to admins");
    let admin_settings = AdminSettings::build(&settings.admins);
    attach_stops_to_admin(stops.iter_mut(), &admin_settings, client).await?;

    tracing::info!("Beginning to import stops into elasticsearch.");
    import_stops(client, &settings.container, futures::stream::iter(stops)).await
}

// FIXME Should not be ElasticsearchStorage, but rather a trait GenerateIndex
pub async fn import_stops<S>(
    client: &ElasticsearchStorage,
    config: &ContainerConfig,
    stops: S,
) -> Result<(), Error>
where
    S: Stream<Item = Stop> + Send + Sync + Unpin + 'static,
{
    client
        .generate_index(config, stops)
        .await
        .context(IndexGenerationSnafu)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::stops::make_weight;
    use cosmogony::ZoneType;
    use places::{admin::Admin, stop::Stop};
    use serial_test::serial;
    use std::{collections::HashMap, sync::Arc};

    fn approx_equal(a: f64, b: f64, dp: u8) -> bool {
        let p = 10f64.powi(-(dp as i32));
        (a - b).abs() < p
    }

    #[tokio::test]
    #[serial]
    async fn test_make_weight_without_physical_mode_weight_and_without_admins() {
        let mut stop = Stop {
            id: "123".to_string(),
            ..Default::default()
        };
        make_weight(&mut stop, &HashMap::new());
        assert!(approx_equal(stop.weight, 0.0, 1));
    }

    #[tokio::test]
    #[serial]
    async fn test_make_weight_without_physical_mode_weight_and_with_admins() {
        let admin = Admin {
            id: "adm:01".to_string(),
            weight: 0.12,
            zone_type: Some(ZoneType::City),
            ..Default::default()
        };
        let mut stop = Stop {
            id: "123".to_string(),
            administrative_regions: vec![Arc::new(admin)],
            ..Default::default()
        };
        make_weight(&mut stop, &HashMap::new());
        approx_equal(stop.weight, 2.0930, 4);
    }

    #[tokio::test]
    #[serial]
    async fn test_make_weight_with_physical_mode_weight_and_with_admins() {
        let mut physical_mode_weight = HashMap::new();
        physical_mode_weight.insert("123".to_string(), 5.0);

        let admin = Admin {
            id: "adm:01".to_string(),
            weight: 0.12,
            zone_type: Some(ZoneType::City),
            ..Default::default()
        };
        let mut stop = Stop {
            id: "123".to_string(),
            administrative_regions: vec![Arc::new(admin)],
            ..Default::default()
        };
        make_weight(&mut stop, &physical_mode_weight);
        approx_equal(stop.weight, 3.5465, 4);
    }
}
