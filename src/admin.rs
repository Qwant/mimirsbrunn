// Copyright © 2018, Hove and/or its affiliates. All rights reserved.
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

use cosmogony::{Zone, ZoneIndex, ZoneType::City};
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use mimir::domain::model::configuration::ContainerConfig;
use mimir::domain::ports::primary::list_documents::ListDocuments;
use snafu::{ResultExt, Snafu};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Arc,
};
use tracing::{info, warn};

use crate::{
    osm_reader::{admin, osm_utils},
    settings::admin_settings::{AdminFromCosmogonyFile, AdminSettings},
};
use mimir::{
    adapters::secondary::elasticsearch::{self, ElasticsearchStorage},
    domain::ports::primary::generate_index::GenerateIndex,
};
use places::admin::Admin;

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

    #[snafu(display("Cosmogony Error: {}", source))]
    Cosmogony { source: anyhow::Error },

    #[snafu(display("Index Generation Error {}", source))]
    IndexGeneration {
        source: mimir::domain::model::error::Error,
    },

    #[snafu(display("Admin Retrieval Error {}", source))]
    AdminRetrieval {
        source: mimir::domain::model::error::Error,
    },

    #[snafu(display("No admins were retrieved from ES"))]
    NoImportedAdmins,
}

trait IntoAdmin {
    fn into_admin(
        self,
        _: &BTreeMap<ZoneIndex, (String, Option<String>)>,
        langs: &[String],
        max_weight: f64,
        french_id_retrocompatibility: bool,
        all_admins: Option<&HashMap<String, Arc<Admin>>>,
    ) -> Admin;
}

// FIXME Should not be ElasticsearchStorage, but rather a trait GenerateIndex
pub async fn import_admins<S>(
    client: &ElasticsearchStorage,
    config: &ContainerConfig,
    admins: S,
) -> Result<(), Error>
where
    S: Stream<Item = Admin> + Send + Sync + Unpin + 'static,
{
    let _ = client
        .generate_index(config, admins)
        .await
        .context(IndexGenerationSnafu)?;
    Ok(())
}

fn get_weight(tags: &osmpbfreader::Tags, center_tags: &osmpbfreader::Tags) -> f64 {
    // to have an admin weight we use the osm 'population' tag to priorize
    // the big zones over the small one.
    // Note: this tags is not often filled , so only some zones
    // will have a weight (but the main cities have it).
    tags.get("population")
        .and_then(|p| p.parse().ok())
        .or_else(|| center_tags.get("population")?.parse().ok())
        .unwrap_or(0.)
}

impl IntoAdmin for Zone {
    fn into_admin(
        self,
        zones_osm_id: &BTreeMap<ZoneIndex, (String, Option<String>)>,
        langs: &[String],
        max_weight: f64,
        french_id_retrocompatibility: bool,
        all_admins: Option<&HashMap<String, Arc<Admin>>>,
    ) -> Admin {
        let insee = admin::read_insee(&self.tags).map(|s| s.to_owned());
        let zip_codes = admin::read_zip_codes(&self.tags);
        let label = self.label;
        let weight = get_weight(&self.tags, &self.center_tags);
        let center = self.center.map_or(places::coord::Coord::default(), |c| {
            places::coord::Coord::new(c.x(), c.y()).expect("invalid coordinate for admin")
        });
        let format_id = |id, insee| {
            // for retrocompatibity reasons, Navitia needs the
            // french admins to have an id with the insee for cities
            match insee {
                Some(insee) if french_id_retrocompatibility => format!("admin:fr:{}", insee),
                _ => format!("admin:osm:{}", id),
            }
        };
        let parent_osm_id = self
            .parent
            .and_then(|id| zones_osm_id.get(&id))
            .map(|(id, insee)| format_id(id, insee.as_ref()));
        let codes = osm_utils::get_osm_codes_from_tags(&self.tags);
        let mut admin = Admin {
            id: zones_osm_id
                .get(&self.id)
                .map(|(id, insee)| format_id(id, insee.as_ref()))
                .expect("unable to find zone id in zones_osm_id"),
            insee: insee.unwrap_or_else(|| "".to_owned()),
            level: self.admin_level.unwrap_or(0),
            label,
            name: self.name,
            alt_name: Option::from(self.alt_name),
            loc_name: Option::from(self.loc_name),
            zip_codes,
            weight: places::admin::normalize_weight(weight, max_weight),
            bbox: self.bbox,
            boundary: self.boundary,
            coord: center,
            approx_coord: Some(center.into()),
            zone_type: self.zone_type,
            parent_id: parent_osm_id,
            // Note: Since we do not really attach an admin to its hierarchy, for the moment an admin only have it's own coutry code,
            // not the country code of it's country from the hierarchy
            // (so it has a country code mainly if it is a country)
            country_codes: places::utils::get_country_code(&codes)
                .into_iter()
                .collect(),
            codes,
            names: osm_utils::get_label_languages_from_tags(&self.tags, "name:", langs),
            alt_names: osm_utils::get_label_languages_from_tags(&self.tags, "alt_name:", langs),
            loc_names: osm_utils::get_label_languages_from_tags(&self.tags, "loc_name:", langs),
            labels: self
                .international_labels
                .into_iter()
                .filter(|(k, _)| langs.contains(k))
                .collect(),
            distance: None,
            context: None,
            administrative_regions: Vec::new(),
        };
        if let Some(admins) = all_admins {
            // Get a list of encompassing parent ids, which will be used as the get
            // administrative_regions.
            let mut parent_ids = Vec::new();
            let mut current = &admin;
            while current.parent_id.is_some() {
                parent_ids.push(current.parent_id.clone().unwrap());
                if let Some(par) = admins.get(parent_ids.last().unwrap()) {
                    current = par;
                } else {
                    break;
                }
            }
            admin.administrative_regions = parent_ids
                .into_iter()
                .filter_map(|a| admins.get(&a))
                .map(Arc::clone)
                .collect::<Vec<_>>();
        }
        admin
    }
}

fn read_zones(path: &Path) -> Result<impl Iterator<Item = Zone> + Send + Sync, Error> {
    let iter = cosmogony::read_zones_from_file(path)
        .context(CosmogonySnafu)?
        .filter_map(|r| r.map_err(|e| warn!("impossible to read zone: {}", e)).ok());
    Ok(iter)
}

pub async fn index_cosmogony(
    path: &Path,
    langs: Vec<String>,
    config: &ContainerConfig,
    french_id_retrocompatibility: bool,
    client: &ElasticsearchStorage,
) -> Result<(), Error> {
    let file_config = AdminFromCosmogonyFile {
        cosmogony_file: path.to_path_buf(),
        langs,
        french_id_retrocompatibility,
    };
    let admins = read_admin_in_cosmogony_file(&file_config)?;
    import_admins(client, config, futures::stream::iter(admins)).await
}

pub async fn fetch_admins(
    admin_settings: &AdminSettings,
    client: &ElasticsearchStorage,
) -> Result<Vec<Admin>, Error> {
    match admin_settings {
        AdminSettings::Elasticsearch => fetch_admin_from_elasticsearch(client).await,
        AdminSettings::Local(config) => {
            let admin_iter = read_admin_in_cosmogony_file(config)?;
            let admins: Vec<Admin> = admin_iter.collect();
            Ok(admins)
        }
    }
}

pub fn read_admin_in_cosmogony_file(
    config: &AdminFromCosmogonyFile,
) -> Result<impl Iterator<Item = Admin>, Error> {
    let path = &config.cosmogony_file;
    let langs = config.langs.clone();
    let french_id_retrocompatibility = config.french_id_retrocompatibility;
    info!("building map cosmogony id => osm id");
    let mut cosmogony_id_to_osm_id = BTreeMap::new();
    let max_weight = places::admin::ADMIN_MAX_WEIGHT;
    for z in read_zones(path)? {
        let insee = match z.zone_type {
            Some(City) => admin::read_insee(&z.tags).map(|s| s.to_owned()),
            _ => None,
        };
        cosmogony_id_to_osm_id.insert(z.id, (z.osm_id.clone(), insee));
    }
    let cosmogony_id_to_osm_id = cosmogony_id_to_osm_id;

    info!("building admins hierarchy");
    let admins_without_boundaries = read_zones(path)?
        .map(|mut zone| {
            zone.boundary = None;
            let admin = zone.into_admin(
                &cosmogony_id_to_osm_id,
                &langs,
                max_weight,
                french_id_retrocompatibility,
                None,
            );
            (admin.id.clone(), Arc::new(admin))
        })
        .collect::<HashMap<_, _>>();

    let admins = read_zones(path)?.map(move |z| {
        z.into_admin(
            &cosmogony_id_to_osm_id,
            &langs,
            max_weight,
            french_id_retrocompatibility,
            Some(&admins_without_boundaries),
        )
    });
    Ok(admins)
}

/// Load admins from mimir's ElasticSearch. The resulting memory usage will be lower that naively
/// deserializing admins from ES as each admin will be only instantiated once and all its children
/// will use a shared pointer to it.
async fn fetch_admin_from_elasticsearch<C: ListDocuments<Admin>>(
    client: &C,
) -> Result<Vec<Admin>, Error> {
    let mut admins_cache: HashMap<String, Arc<Admin>> = HashMap::new();

    let admins: Vec<_> = client
        .list_documents()
        .await
        .context(AdminRetrievalSnafu)?
        .map(|admin| {
            let mut admin = admin?;

            admin.administrative_regions.iter_mut().for_each(|parent| {
                if let Some(cached_parent) = admins_cache.get(&parent.id) {
                    *parent = cached_parent.clone();
                } else {
                    admins_cache.insert(parent.id.clone(), parent.clone());
                }
            });

            Ok(admin)
        })
        .try_collect()
        .await
        .context(IndexGenerationSnafu)?;

    if admins.is_empty() {
        return Err(Error::NoImportedAdmins);
    }

    info!("{} admins retrieved from ES ", admins.len());
    Ok(admins)
}
