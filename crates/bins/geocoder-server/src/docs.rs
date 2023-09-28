use std::sync::Arc;

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

use crate::errors::AppError;
use crate::extractors::Json;

pub fn docs_routes() -> ApiRouter {
    aide::gen::infer_responses(true);

    let router = ApiRouter::new()
        .api_route_with(
            "/",
            get_with(
                Redoc::new("/docs/private/api.json")
                    .with_title("Qwant geocoder")
                    .axum_handler(),
                |op| op.description("This documentation page.").hidden(true),
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
    api.title("Geocoder API")
        .summary("Qwant geocoder API")
        .description(include_str!("../README.md"))
        .default_response_with::<Json<AppError>, _>(|res| {
            res.example(AppError {
                error: "some error happened".to_string(),
                error_details: None,
                error_id: Uuid::nil(),
                status: StatusCode::INTERNAL_SERVER_ERROR,
            })
        })
}
