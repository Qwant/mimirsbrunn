use std::sync::Arc;

use aide::openapi::Tag;
use aide::transform::TransformOpenApi;
use aide::{
    axum::{
        routing::{get, get_with},
        ApiRouter, IntoApiResponse,
    },
    openapi::OpenApi,
    redoc::Redoc,
};
use axum::http::StatusCode;
use axum::{response::IntoResponse, Extension};
use uuid::Uuid;

use axum_common::error::AppError;
use axum_common::extract::json::Json;

pub fn docs_routes() -> ApiRouter {
    aide::gen::infer_responses(true);

    let router = ApiRouter::new()
        .api_route_with(
            "/",
            get_with(
                Redoc::new("/docs/private/api.json")
                    .with_title("Tagger API")
                    .axum_handler(),
                |op| op.description("This documentation page."),
            ),
            |p| p,
        )
        .route("/private/api.json", get(serve_docs));

    aide::gen::infer_responses(false);

    router
}

async fn serve_docs(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoApiResponse {
    Json(api).into_response()
}

pub fn api_docs(api: TransformOpenApi) -> TransformOpenApi {
    api.title("Tagger API")
        .summary("Query tagger for Qwant map")
        .description(include_str!("../README.md"))
        .tag(Tag {
            name: "TaggerPart".into(),
            description: Some("A tagged word token".into()),
            ..Default::default()
        })
        .default_response_with::<Json<AppError>, _>(|res| {
            res.example(AppError {
                error: "some error happened".to_string(),
                error_details: None,
                error_id: Uuid::nil(),
                status: StatusCode::INTERNAL_SERVER_ERROR,
            })
        })
}
