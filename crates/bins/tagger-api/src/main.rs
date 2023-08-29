use crate::docs::{api_docs, docs_routes};
use crate::dto::{TaggedPartDto, TaggerResponseLegacy};
use crate::extractors::Json;
use crate::override_legacy::tag_legacy;
use aide::axum::routing::{get_with, post_with};
use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use aide::transform::TransformOperation;
use autometrics::prometheus_exporter::PrometheusResponse;
use autometrics::{autometrics, prometheus_exporter};
use axum::extract::Query;
use axum::routing::get;
use axum::Extension;
use clap::Parser;
use reqwest::Client;
use schemars::JsonSchema;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tagger::{TaggerQueryBuilder, ASSETS_PATH};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub mod docs;
pub mod dto;
pub mod errors;
pub mod extractors;
pub mod override_legacy;

#[derive(Debug, Parser)]
#[clap(name = "query", about = "Tagger-API")]
struct Cli {
    /// Activate debug mode
    #[clap(short, long, env)]
    debug: bool,

    /// Path to assets (see: ./libs/tagger/assets)
    #[clap(short, long, env)]
    assets_path: Option<PathBuf>,

    /// http port to bind
    #[clap(long, short, env)]
    port: Option<u16>,

    /// http port to bind
    #[clap(long, short, env)]
    legacy_tagger_url: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    legacy_tagger_url: String,
    client: Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let loglevel = if cli.debug {
        "tagger_api=debug,tower_http=debug"
    } else {
        "tagger_api=info,tower_http=info"
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| loglevel.into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Some(assets) = cli.assets_path {
        ASSETS_PATH.get_or_init(|| assets);
    };

    let state = AppState {
        legacy_tagger_url: cli.legacy_tagger_url,
        client: Default::default(),
    };

    prometheus_exporter::init();

    // lazy load the tagger file once
    info!("Loading assets ...");
    TaggerQueryBuilder::all().apply_taggers("dummy");

    aide::gen::on_error(|error| {
        println!("{error}");
    });

    aide::gen::extract_schemas(true);

    let mut api = OpenApi::default();

    let router = ApiRouter::new()
        .api_route("/tagger-new", get_with(tag, tag_docs))
        .api_route("/tagger", post_with(tag_legacy, tag_legacy_docs))
        .route("/metrics", get(get_metrics))
        .nest_api_service("/docs", docs_routes())
        .finish_api_with(&mut api, api_docs)
        .with_state(state)
        .layer(Extension(Arc::new(api)))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port.unwrap_or(7001)));
    info!("Staring tagger-api on http://{addr}");
    info!(
        "Openapi docs are accessible at http://{}/docs",
        addr.to_string()
    );
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct TaggerQuery {
    text: String,
}

#[autometrics]
async fn tag(Query(query): Query<TaggerQuery>) -> Json<Vec<TaggedPartDto>> {
    Json(
        TaggerQueryBuilder::all()
            .apply_taggers(&query.text)
            .into_iter()
            .map(TaggedPartDto::from)
            .collect(),
    )
}

fn tag_docs(op: TransformOperation) -> TransformOperation {
    op.description("Tag the given user query")
        .response::<200, Json<Vec<TaggedPartDto>>>()
}

fn tag_legacy_docs(op: TransformOperation) -> TransformOperation {
    op.description("Tag the given user query")
        .response::<200, Json<TaggerResponseLegacy>>()
}

pub async fn get_metrics() -> PrometheusResponse {
    prometheus_exporter::encode_http_response()
}
