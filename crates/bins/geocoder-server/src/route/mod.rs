use crate::errors::AppError;
use crate::settings::Settings;
use crate::AppState;
use autometrics::autometrics;
use elastic_client::model::query::Query as ElasticQuery;
use elastic_query_builder::dsl::QueryType;
use elastic_query_builder::filters;
use elastic_query_builder::filters::Filters;
use elastic_query_builder::geocoding::{Feature, FromWithLang};
use elastic_query_builder::indices::build_es_indices_to_search;
use elastic_query_builder::query::ForwardGeocoderQuery;
use elastic_query_builder::settings::QuerySettings;
use geo::HaversineDistance;
use places::coord::Coord;
use places::Place;
use serde_json::Value;
use std::time::Duration;

pub mod autocomplete;
pub mod explain;
pub mod reverse;
pub mod search;
pub mod status;

fn get_search_fields_from_params(
    settings: &Settings,
    params: ForwardGeocoderQuery,
) -> (
    String,
    Duration,
    Vec<String>,
    String,
    Filters,
    [String; 1],
    QuerySettings,
    bool,
) {
    let q = params.q.clone();
    let timeout = params.timeout.unwrap_or(settings.autocomplete_timeout);

    let es_indices_to_search_in = build_es_indices_to_search(
        &settings.elasticsearch.index_root,
        &params.types,
        &params.poi_dataset,
    );

    let lang = params.lang.clone();
    let is_exact_match = params.is_exact_match;
    let filters = filters::Filters::from(params);
    let excludes = ["boundary".to_string()];
    let settings_query = &settings.query;

    (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        settings_query.clone(),
        is_exact_match,
    )
}

#[autometrics]
async fn request_search_documents(
    state: &AppState,
    timeout: Duration,
    es_indices_to_search_in: Vec<String>,
    results_limit: i64,
    query_type: QueryType,
    dsl_query: Value,
) -> Result<Vec<Place>, AppError> {
    tracing::trace!(
        query_type = ?query_type,
        indices = ?es_indices_to_search_in,
        query = tracing::field::display(dsl_query.to_string()),
        "Query ES",
    );

    let res = state
        .client
        .search_documents(
            es_indices_to_search_in.clone(),
            ElasticQuery::QueryDSL(dsl_query),
            results_limit,
            Some(timeout),
        )
        .await;

    res.map_err(AppError::from)
}

pub fn build_feature(
    places: Vec<Place>,
    query_coord: Option<&Coord>,
    lang: Option<&str>,
) -> Vec<Feature> {
    places
        .into_iter()
        .map(|mut p| {
            if let Some(coord) = query_coord {
                let geo_point = geo::Point::new(coord.lon(), coord.lat());
                let pp: geo::Point<f64> = geo::Point::new(p.coord().lon(), p.coord().lat());
                let distance = geo_point.haversine_distance(&pp) as u32;
                p.set_distance(distance);
            }
            Feature::from_with_lang(p, lang)
        })
        .collect()
}
