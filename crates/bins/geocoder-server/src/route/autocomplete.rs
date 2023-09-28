use crate::errors::AppError;
use crate::extractors::Json;
use crate::route::{build_feature, get_search_fields_from_params, request_search_documents};
use crate::AppState;
use aide::transform::TransformOperation;
use autometrics::autometrics;
use axum::extract::{Query, State};
use elastic_query_builder::dsl;
use elastic_query_builder::dsl::QueryType;
use elastic_query_builder::geocoding::GeocodeJsonResponse;
use elastic_query_builder::query::ForwardGeocoderQuery;
use tracing::instrument;

#[instrument(skip(state))]
#[autometrics]
pub async fn autocomplete(
    State(state): State<AppState>,
    Query(params): Query<ForwardGeocoderQuery>,
) -> Result<Json<GeocodeJsonResponse>, AppError> {
    let (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        query_settings,
        _is_exact_match,
    ) = get_search_fields_from_params(&state.settings, params);

    for query_type in [QueryType::PREFIX, QueryType::FUZZY] {
        let dsl_query = dsl::build_query(
            &state.settings.elasticsearch.index_root,
            &q,
            &filters,
            lang.as_str(),
            &query_settings,
            query_type,
            Some(&excludes),
            false,
        );

        let places = request_search_documents(
            &state,
            timeout,
            es_indices_to_search_in.clone(),
            filters.limit,
            query_type,
            dsl_query,
        )
        .await?;

        if !places.is_empty() {
            let features = build_feature(places, filters.coord.as_ref(), Some(lang.as_str()));
            let resp = GeocodeJsonResponse::new(q, features);
            return Ok(Json(resp));
        }
    }

    Ok(Json(GeocodeJsonResponse::new(q, vec![])))
}

pub fn autocomplete_docs(op: TransformOperation) -> TransformOperation {
    op.description("Autocomplete geocoding query")
        .response::<200, Json<Vec<GeocodeJsonResponse>>>()
}
