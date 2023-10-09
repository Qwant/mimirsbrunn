use aide::transform::TransformOperation;
use autometrics::autometrics;
use axum::extract::State;
use axum_macros::debug_handler;
use http::StatusCode;
use tracing::instrument;

use axum_common::extract::json::Json;
use axum_common::extract::query::ValidatedQuery;
use elastic_client::model::query::Query as ElasticQuery;
use elastic_query_builder::doc_type::root_doctype;
use elastic_query_builder::dsl;
use elastic_query_builder::geocoding::{FromWithLang, GeocodeJsonResponse};
use elastic_query_builder::query::ReverseGeocoderQuery;
use places::addr::Addr;
use places::street::Street;
use places::ContainerDocument;

use crate::AppState;
use axum_common::error::AppError;

#[debug_handler]
#[instrument(skip(state))]
#[autometrics]
pub async fn reverse_geocode(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<ReverseGeocoderQuery>,
) -> Result<Json<GeocodeJsonResponse>, AppError> {
    let timeout = query.timeout.unwrap_or(state.settings.autocomplete_timeout);
    let distance = format!("{}m", state.settings.query.reverse_query.radius);
    let dsl = dsl::build_reverse_query(&distance, query.lat, query.lon);

    let es_indices_to_search_in = vec![
        root_doctype(
            &state.settings.elasticsearch.index_root,
            Street::static_doc_type(),
        ),
        root_doctype(
            &state.settings.elasticsearch.index_root,
            Addr::static_doc_type(),
        ),
    ];

    tracing::trace!(
        "Searching in indexes {:?} with query {}",
        es_indices_to_search_in,
        serde_json::to_string_pretty(&dsl).unwrap()
    );

    let places = state
        .client
        .search_documents(
            es_indices_to_search_in,
            ElasticQuery::QueryDSL(dsl),
            query.limit,
            Some(timeout),
        )
        .await
        .map_err(|_err| {
            AppError::new("Elastic search error").with_status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    Ok(Json(GeocodeJsonResponse::from_with_lang(places, None)))
}

pub fn reverse_geocode_docs(op: TransformOperation) -> TransformOperation {
    op.description("Reverse geocoding query")
        .response::<200, Json<Vec<GeocodeJsonResponse>>>()
}
