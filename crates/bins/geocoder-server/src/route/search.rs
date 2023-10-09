use aide::transform::TransformOperation;
use autometrics::autometrics;
use axum::extract::State;
use axum_macros::debug_handler;
use tracing::instrument;

use axum_common::error::AppError;
use axum_common::extract::json::Json;
use axum_common::extract::query::ValidatedQuery;
use elastic_query_builder::dsl;
use elastic_query_builder::dsl::QueryType;
use elastic_query_builder::geocoding::GeocodeJsonResponse;
use elastic_query_builder::query::GeocoderQuery;

use crate::route::{build_feature, get_search_fields_from_params, request_search_documents};
use crate::AppState;

#[debug_handler]
#[instrument(skip(state))]
#[autometrics]
pub async fn search(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<GeocoderQuery>,
) -> Result<Json<GeocodeJsonResponse>, AppError> {
    let (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        query_settings,
        is_exact_match,
    ) = get_search_fields_from_params(&state.settings.clone(), query);

    let dsl_query = dsl::build_query(
        &state.settings.elasticsearch.index_root,
        &q,
        &filters,
        lang.as_str(),
        &query_settings,
        QueryType::SEARCH,
        Some(&excludes),
        is_exact_match,
    );

    let places = request_search_documents(
        &state,
        timeout,
        es_indices_to_search_in.clone(),
        1,
        QueryType::SEARCH,
        dsl_query,
    )
    .await?;

    if !places.is_empty() {
        let features = build_feature(places, filters.coord.as_ref(), Some(lang.as_str()));
        let resp = GeocodeJsonResponse::new(q, features);
        return Ok(Json(resp));
    }

    Ok(Json(GeocodeJsonResponse::new(q, vec![])))
}

pub fn search_docs(op: TransformOperation) -> TransformOperation {
    op.description("Search geocoding query")
        .response::<200, Json<Vec<GeocodeJsonResponse>>>()
}
