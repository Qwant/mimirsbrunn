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
// WITHOUT ANY WA&RRANTY; without even the implied warranty of
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
use futures::stream::{Stream, StreamExt};
use mimir::domain::model::configuration::root_doctype;
use navitia_poi_model::{Model as NavitiaModel, Poi as NavitiaPoi, PoiType as NavitiaPoiType};
use snafu::{ResultExt, Snafu};
use std::{collections::HashMap, ops::Deref, path::PathBuf, sync::Arc};
use tracing::instrument;

use crate::{
    admin,
    admin_geofinder::AdminGeoFinder,
    labels,
    settings::{self, admin_settings::AdminSettings},
};
use common::document::ContainerDocument;
use mimir::{
    adapters::{
        primary::common::dsl,
        secondary::elasticsearch::{self, ElasticsearchStorage},
    },
    domain::{
        model::{configuration::ContainerConfig, query::Query},
        ports::primary::{generate_index::GenerateIndex, search_documents::SearchDocuments},
    },
};
use places::{
    addr::Addr,
    i18n_properties::I18nProperties,
    poi::{Poi, PoiType},
    street::Street,
    Place,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchPool {
        source: elasticsearch::remote::Error,
    },

    #[snafu(display("Index Generation Error {}", source))]
    IndexGeneration {
        source: mimir::domain::model::error::Error,
    },

    #[snafu(display("Reverse Addres Search Error {}", source))]
    ReverseAddressSearch {
        source: mimir::domain::model::error::Error,
    },

    #[snafu(display("Invalid JSON: {} ({})", source, details))]
    Json {
        details: String,
        source: serde_json::Error,
    },

    // navitia_poi_model uses failure::Error, which does not implement std::Error, so
    // we use a String to get the error message instead.
    #[snafu(display("Navitia Model Error {}", details))]
    NavitiaModelExtraction { details: String },

    #[snafu(display("Unrecognized Poi Type {}", details))]
    UnrecognizedPoiType { details: String },

    #[snafu(display("No Address Found {}", details))]
    NoAddressFound { details: String },

    #[snafu(display("No Admin Found {}", details))]
    NoAdminFound { details: String },

    #[snafu(display("Admin Retrieval Error {}", details))]
    AdminRetrieval { details: admin::Error },
}

/// Stores the pois found in the 'input' file, in Elasticsearch, with the given configuration.
/// We extract the list of pois from the input file, which is in the Navtia Model format.
/// We then enrich this list, before importing it.
#[instrument(skip_all)]
pub async fn index_pois(
    input: PathBuf,
    client: &ElasticsearchStorage,
    settings: settings::poi2mimir::Settings,
) -> Result<(), Error> {
    let NavitiaModel { pois, poi_types } =
        NavitiaModel::try_from_path(&input).map_err(|err| Error::NavitiaModelExtraction {
            details: format!(
                "Could not read navitia model from {}: {}",
                input.display(),
                err
            ),
        })?;

    let admin_settings = AdminSettings::build(&settings.admins);

    let admins_geofinder = AdminGeoFinder::build(&admin_settings, client)
        .await
        .map_err(|err| Error::AdminRetrieval { details: err })?;

    let admins_geofinder = Arc::new(admins_geofinder);

    let poi_types = Arc::new(poi_types);

    let pois: Vec<_> = futures::stream::iter(pois.into_iter())
        .then(|(_id, poi)| {
            let poi_types = poi_types.clone();
            let admins_geofinder = admins_geofinder.clone();
            into_poi(
                poi,
                poi_types,
                client,
                admins_geofinder,
                settings.max_distance_reverse,
            )
        })
        .filter_map(|poi_res| futures::future::ready(poi_res.ok()))
        .collect()
        .await;

    import_pois(client, settings.container, futures::stream::iter(pois)).await
}

// FIXME Should not be ElasticsearchStorage, but rather a trait GenerateIndex
pub async fn import_pois<S>(
    client: &ElasticsearchStorage,
    config: ContainerConfig,
    pois: S,
) -> Result<(), Error>
where
    S: Stream<Item = Poi> + Send + Sync + Unpin + 'static,
{
    client
        .generate_index(&config, pois)
        .await
        .context(IndexGenerationSnafu)?;

    Ok(())
}

// This function takes a Poi from the navitia model, ie from the CSV deserialization, and returns
// a Poi from the mimir model, with all the contextual information added.
async fn into_poi(
    poi: NavitiaPoi,
    poi_types: Arc<HashMap<String, NavitiaPoiType>>,
    client: &ElasticsearchStorage,
    admins_geofinder: Arc<AdminGeoFinder>,
    max_distance_reverse: usize,
) -> Result<Poi, Error> {
    let NavitiaPoi {
        id,
        name,
        coord,
        poi_type_id,
        properties,
        visible: _,
        weight: _,
    } = poi;

    let poi_type = poi_types
        .get(&poi_type_id)
        .ok_or(Error::UnrecognizedPoiType {
            details: poi_type_id,
        })
        .map(PoiType::from)?;

    let distance = format!("{}m", max_distance_reverse);
    let dsl = dsl::build_reverse_query(&distance, coord.lat(), coord.lon());

    let es_indices_to_search = vec![
        root_doctype(Street::static_doc_type()),
        root_doctype(Addr::static_doc_type()),
    ];

    let place = client
        .search_documents(es_indices_to_search, Query::QueryDSL(dsl), 1, None)
        .await
        .context(ReverseAddressSearchSnafu)
        .and_then(|values| match values.into_iter().next() {
            None => Ok(None), // If we didn't get any result, return 'no place'
            Some(value) => serde_json::from_value::<Place>(value)
                .context(JsonSnafu {
                    details: "could no deserialize place",
                })
                .map(Some),
        })?;

    let addr = place.as_ref().and_then(|place| {
        let place = place;
        place.address()
    });

    let coord = places::coord::Coord::new(coord.lon(), coord.lat());
    // We the the admins from the address, or, if we don't have any, from the geofinder.
    let admins = place.map_or_else(|| admins_geofinder.get(&coord), |addr| addr.admins());

    if admins.is_empty() {
        return Err(Error::NoAdminFound {
            details: format!("Could not find admins for POI {}", &id),
        });
    }

    // The weight is that of the city, or 0.0 if there is no such admin.
    let weight: f64 = admins
        .iter()
        .filter(|adm| adm.is_city())
        .map(|adm| adm.weight)
        .next()
        .unwrap_or(0.0);

    let country_codes = places::admin::find_country_codes(admins.iter().map(|a| a.deref()));

    let label = labels::format_poi_label(&name, admins.iter().map(|a| a.deref()), &country_codes);

    let poi = Poi {
        id: places::utils::normalize_id("poi", &id),
        label,
        name,
        coord,
        approx_coord: Some(coord.into()),
        administrative_regions: admins,
        weight,
        zip_codes: vec![],
        poi_type,
        properties,
        address: addr,
        country_codes,
        names: I18nProperties::default(),
        labels: I18nProperties::default(),
        distance: None,
        context: None,
    };

    Ok(poi)
}
