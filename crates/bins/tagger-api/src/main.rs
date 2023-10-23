use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use aide::axum::routing::get_with;
use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use aide::transform::TransformOperation;
use autometrics::prometheus_exporter::PrometheusResponse;
use autometrics::settings::AutometricsSettings;
use autometrics::{autometrics, prometheus_exporter};
use axum::extract::Query;
use axum::routing::get;
use axum::Extension;
use clap::Parser;
use schemars::JsonSchema;
use serde::Deserialize;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use axum_common::extract::json::Json;
use tagger::{TaggerQueryBuilder, ASSETS_PATH};

use crate::docs::{api_docs, docs_routes};
use crate::dto::TaggedPartDto;

pub mod docs;
pub mod dto;

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let loglevel = if cli.debug {
        "tagger_api=debug,tower_http=debug"
    } else {
        "tagger_api=info,tower_http=info"
    };

    vergen::EmitBuilder::builder()
        .git_sha(true)
        .git_branch()
        .emit()?;

    AutometricsSettings::builder().service_name("tagger").init();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| loglevel.into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Some(assets) = cli.assets_path {
        ASSETS_PATH.get_or_init(|| assets);
    };

    // lazy load the tagger file once
    info!("Loading assets ...");
    TaggerQueryBuilder::all().apply_taggers("dummy", false);
    TaggerQueryBuilder::all().apply_taggers("dummy", true);

    aide::gen::on_error(|error| {
        println!("{error}");
    });

    aide::gen::extract_schemas(true);

    let mut api = OpenApi::default();

    let router = ApiRouter::new()
        .api_route("/tagger", get_with(tag, tag_docs))
        .api_route(
            "/tagger-autocomplete",
            get_with(tag_autocomplete, tag_autocomplete_docs),
        )
        .route("/metrics", get(get_metrics))
        .nest_api_service("/docs", docs_routes())
        .finish_api_with(&mut api, api_docs)
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
    info!("{:?}", query);
    Json(
        TaggerQueryBuilder::all()
            .apply_taggers(&query.text, false)
            .into_iter()
            .map(TaggedPartDto::from)
            .collect(),
    )
}

#[autometrics]
async fn tag_autocomplete(Query(query): Query<TaggerQuery>) -> Json<Vec<TaggedPartDto>> {
    info!("{:?}", query);
    Json(
        TaggerQueryBuilder::all()
            .apply_taggers(&query.text, true)
            .into_iter()
            .map(TaggedPartDto::from)
            .collect(),
    )
}

fn tag_docs(op: TransformOperation) -> TransformOperation {
    op.description("Tag the given user qwant.com query")
        .response::<200, Json<Vec<TaggedPartDto>>>()
}

fn tag_autocomplete_docs(op: TransformOperation) -> TransformOperation {
    op.description("Tag the given user autocomplete query")
        .response::<200, Json<Vec<TaggedPartDto>>>()
}

pub async fn get_metrics() -> PrometheusResponse {
    prometheus_exporter::encode_http_response()
}
