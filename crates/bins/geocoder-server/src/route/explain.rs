use crate::AppState;
use aide::transform::TransformOperation;
use autometrics::autometrics;
use axum::extract::State;
use axum_common::error::AppError;
use axum_common::extract::json::Json;
use axum_common::extract::query::ValidatedQuery;
use axum_macros::debug_handler;
use elastic_client::model::query::Query as ElasticQuery;
use elastic_query_builder::dsl::QueryType;
use elastic_query_builder::query::ForwardGeocoderExplainQuery;
use elastic_query_builder::{dsl, filters};
use http::StatusCode;
use serde_json::Value;
use tracing::instrument;

#[debug_handler]
#[instrument(skip(state))]
#[autometrics]
pub async fn explain(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<ForwardGeocoderExplainQuery>,
) -> Result<Json<Value>, AppError> {
    let doc_id = query.doc_id.clone();
    let doc_type = query.doc_type.clone();
    let q = query.forward_geocoder_query.q.clone();
    let lang = query.forward_geocoder_query.lang.clone();
    let filters = filters::Filters::from(query.forward_geocoder_query);
    let dsl = dsl::build_query(
        &state.settings.elasticsearch.index_root,
        &q,
        &filters,
        lang.as_str(),
        &state.settings.query,
        QueryType::PREFIX,
        None,
        false,
    );
    match state
        .client
        .explain_search(ElasticQuery::QueryDSL(dsl), doc_id, doc_type)
        .await
    {
        Ok(res) => Ok(Json(res)),
        Err(err) => {
            Err(AppError::new(&err.to_string()).with_status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

pub fn explain_docs(op: TransformOperation) -> TransformOperation {
    op.description("Explain geocoding query")
        .response::<200, Json<Value>>()
}
