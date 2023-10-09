use crate::route::autocomplete::{autocomplete, autocomplete_docs};
use crate::route::reverse::{reverse_geocode, reverse_geocode_docs};
use crate::route::search::{search, search_docs};
use crate::route::status::{status, status_docs};
use crate::settings::{build_settings, Settings};

use crate::docs::{api_docs, docs_routes};
use crate::route::explain::{explain, explain_docs};
use aide::axum::routing::{get_with, post_with};
use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use anyhow::anyhow;
use autometrics::prometheus_exporter;
use autometrics::prometheus_exporter::PrometheusResponse;
use axum::error_handling::HandleErrorLayer;
use axum::routing::get;
use axum::{BoxError, Extension};
use clap::Parser;
use elastic_client::remote::{connection_pool_url, Remote};
use elastic_client::ElasticsearchStorage;
use http::{header, HeaderValue, StatusCode};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod docs;
pub mod errors;
mod extractors;
pub mod route;
mod settings;

#[derive(Debug, Clone)]
pub struct AppState {
    pub client: ElasticsearchStorage,
    pub settings: Settings,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = settings::Opts::parse();
    let settings = build_settings(&opts)?;
    prometheus_exporter::init();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "geocoder_server=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Connecting to Elasticsearch at {}",
        &settings.elasticsearch.url
    );

    let state = AppState {
        client: connection_pool_url(&settings.elasticsearch.url)
            .conn(settings.elasticsearch.clone())
            .await
            .map_err(|err| anyhow!(err.to_string()))?,

        settings: settings.clone(),
    };

    aide::gen::on_error(|error| {
        println!("{error}");
    });

    aide::gen::extract_schemas(true);

    let mut api = OpenApi::default();
    api.info.extensions.insert(
        "x-logo".to_string(),
        json!({
            "url": "https://upload.wikimedia.org/wikipedia/fr/4/46/Qwant_Logo.svg",
            "backgroundColor": "#FFFFFF",
            "altText": "Qwant logo"
        }),
    );

    let timeout_layer =
        ServiceBuilder::new().layer(HandleErrorLayer::new(|error: BoxError| async move {
            if error.is::<tower::timeout::error::Elapsed>() {
                Ok(StatusCode::REQUEST_TIMEOUT)
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {error}"),
                ))
            }
        }));
    info!("Timeout is {:?}", settings.autocomplete_timeout);
    let router = ApiRouter::new()
        .nest(
            "/api/v1",
            ApiRouter::new()
                .api_route("/autocomplete", get_with(autocomplete, autocomplete_docs))
                .layer(timeout_layer.clone().timeout(settings.autocomplete_timeout))
                .api_route("/autocomplete", post_with(autocomplete, autocomplete_docs))
                .layer(timeout_layer.clone().timeout(settings.autocomplete_timeout))
                .api_route("/autocomplete-explain", post_with(explain, explain_docs))
                .api_route("/search", get_with(search, search_docs))
                .api_route("/reverse", get_with(reverse_geocode, reverse_geocode_docs))
                .layer(timeout_layer.clone().timeout(settings.reverse_timeout))
                .api_route("/status", get_with(status, status_docs)),
        )
        .nest_api_service("/docs", docs_routes())
        .with_state(state)
        .finish_api_with(&mut api, api_docs)
        .layer(Extension(Arc::new(api)))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::try_from(format!("max-age={}", settings.http_cache_duration))?,
        ))
        .layer(TraceLayer::new_for_http())
        .route("/api/v1/metrics", get(get_metrics));

    let addr = SocketAddr::from(([0, 0, 0, 0], settings.service.port));

    info!("Staring geocoder on http://{addr}");
    info!(
        "Openapi docs are accessible at http://{}/docs",
        addr.to_string()
    );
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}

pub async fn get_metrics() -> PrometheusResponse {
    prometheus_exporter::encode_http_response()
}
