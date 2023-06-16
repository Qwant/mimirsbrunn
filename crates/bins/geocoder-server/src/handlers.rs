use std::time::Duration;

use geo::algorithm::haversine_distance::HaversineDistance;
use geojson::Geometry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::instrument;
use warp::http::StatusCode;
use warp::reject::Reject;
use warp::reply::{json, with_status};
use warp::Rejection;

use crate::api::{BragiStatus, ElasticsearchStatus, MimirStatus, StatusResponseBody};
use crate::prometheus_handler;
use elastic_client::model::query::Query;
use elastic_client::{ElasticsearchStorage, ElasticsearchStorageConfig};
use elastic_query_builder::doc_type::root_doctype;
use elastic_query_builder::dsl::QueryType;
use elastic_query_builder::filters::{Filters, Proximity};
use elastic_query_builder::geocoding::{Feature, FromWithLang, GeocodeJsonResponse};
use elastic_query_builder::indices::build_es_indices_to_search;
use elastic_query_builder::query::{
    ForwardGeocoderExplainQuery, ForwardGeocoderParamsQuery, ForwardGeocoderQuery,
    ReverseGeocoderQuery,
};
use elastic_query_builder::settings::QuerySettings;
use elastic_query_builder::{coord, dsl, filters};
use places::addr::Addr;
use places::street::Street;
use places::{ContainerDocument, Place};
use serde_helpers::deserialize_duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "metrics")]
lazy_static::lazy_static! {
    static ref ES_REQ_HISTOGRAM: prometheus::HistogramVec = prometheus::register_histogram_vec!(
        "bragi_elasticsearch_request_duration_seconds",
        "The elasticsearch request latencies in seconds.",
        &["search_type"],
        prometheus::exponential_buckets(0.001, 1.5, 25).unwrap()
    )
    .unwrap();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Host on which we expose bragi. Example: 'http://localhost', '0.0.0.0'
    pub host: String,
    /// Port on which we expose bragi.
    pub port: u16,
    /// Used on POST request to set an upper limit on the size of the body (in bytes)
    pub content_length_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: String,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub query: QuerySettings,
    pub service: Service,
    pub nb_threads: Option<usize>,
    pub http_cache_duration: usize,
    #[serde(deserialize_with = "deserialize_duration")]
    pub autocomplete_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub reverse_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub features_timeout: Duration,
}

#[derive(Clone)]
pub struct Context {
    pub client: ElasticsearchStorage,
    pub settings: Settings,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum InternalErrorReason {
    ElasticSearchError,
    SerializationError,
    ObjectNotFoundError,
    StatusError,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct InternalError {
    pub reason: InternalErrorReason,
    pub info: String,
}

impl Reject for InternalError {}

pub fn build_feature(
    places: Vec<Place>,
    query_coord: Option<&coord::Coord>,
    lang: Option<&str>,
) -> Vec<Feature> {
    places
        .into_iter()
        .map(|mut p| {
            if let Some(coord) = query_coord {
                let geo_point = geo::Point::new(coord.lon as f64, coord.lat as f64);
                let pp: geo::Point<f64> = geo::Point::new(p.coord().lon(), p.coord().lat());
                let distance = geo_point.haversine_distance(&pp) as u32;
                p.set_distance(distance);
            }
            Feature::from_with_lang(p, lang)
        })
        .collect()
}

#[instrument(skip(ctx))]
pub async fn forward_autocomplete_geocoder(
    ctx: Context,
    params: ForwardGeocoderParamsQuery,
    geometry: Option<Geometry>,
) -> Result<impl warp::Reply, Rejection> {
    let (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        query_settings,
        _is_exact_match,
    ) = get_search_fields_from_params(
        ctx.settings.clone(),
        params.forward_geocoder_query,
        geometry,
        params.proximity,
    );

    for query_type in [QueryType::PREFIX, QueryType::FUZZY] {
        let dsl_query = dsl::build_query(
            &ctx.settings.elasticsearch.index_root,
            &q,
            &filters,
            lang.as_str(),
            &query_settings,
            query_type,
            Some(&excludes),
            false,
        );

        let places = request_search_documents(
            &ctx,
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
            return Ok(with_status(json(&resp), StatusCode::OK));
        }
    }

    Ok(with_status(
        json(&GeocodeJsonResponse::new(q, vec![])),
        StatusCode::OK,
    ))
}

#[instrument(skip(ctx))]
pub async fn forward_search_geocoder(
    ctx: Context,
    params: ForwardGeocoderParamsQuery,
    geometry: Option<Geometry>,
) -> Result<impl warp::Reply, Rejection> {
    let (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        query_settings,
        is_exact_match,
    ) = get_search_fields_from_params(
        ctx.settings.clone(),
        params.forward_geocoder_query,
        geometry,
        params.proximity,
    );

    let dsl_query = dsl::build_query(
        &ctx.settings.elasticsearch.index_root,
        &q,
        &filters,
        lang.as_str(),
        &query_settings,
        QueryType::SEARCH,
        Some(&excludes),
        is_exact_match,
    );

    let places = request_search_documents(
        &ctx,
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
        Ok(with_status(json(&resp), StatusCode::OK))
    } else {
        Ok(with_status(
            json(&GeocodeJsonResponse::new(q, vec![])),
            StatusCode::OK,
        ))
    }
}

async fn request_search_documents(
    ctx: &Context,
    timeout: Duration,
    es_indices_to_search_in: Vec<String>,
    results_limit: i64,
    query_type: QueryType,
    dsl_query: Value,
) -> Result<Vec<Place>, Rejection> {
    tracing::trace!(
        query_type = ?query_type,
        indices = ?es_indices_to_search_in,
        query = tracing::field::display(dsl_query.to_string()),
        "Query ES",
    );

    #[cfg(feature = "metrics")]
    let timer = ES_REQ_HISTOGRAM
        .get_metric_with_label_values(&[query_type.as_str()])
        .map(|h| h.start_timer())
        .map_err(|err| {
            tracing::error_span!(
                "impossible to get ES_REQ_HISTOGRAM metrics",
                err = err.to_string().as_str()
            )
        })
        .ok();

    let res = ctx
        .client
        .search_documents(
            es_indices_to_search_in.clone(),
            Query::QueryDSL(dsl_query),
            results_limit,
            Some(timeout),
        )
        .await;

    #[cfg(feature = "metrics")]
    if let Some(timer) = timer {
        timer.observe_duration();
    }

    let places: Result<Vec<Place>, Rejection> = res.map_err(|err| {
        warp::reject::custom(InternalError {
            reason: InternalErrorReason::ElasticSearchError,
            info: err.to_string(),
        })
    });
    places
}

fn get_search_fields_from_params(
    settings: Settings,
    params: ForwardGeocoderQuery,
    geometry: Option<Geometry>,
    proximity: Option<Proximity>,
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
        &params.pt_dataset,
        &params.poi_dataset,
    );

    let lang = params.lang.clone();
    let is_exact_match = params.is_exact_match;
    let filters = filters::Filters::from((params, geometry, proximity));
    let excludes = ["boundary".to_string()];
    let settings_query = settings.query;

    (
        q,
        timeout,
        es_indices_to_search_in,
        lang,
        filters,
        excludes,
        settings_query,
        is_exact_match,
    )
}

#[instrument(skip(ctx))]
pub async fn forward_geocoder_explain(
    ctx: Context,
    params: ForwardGeocoderExplainQuery,
    geometry: Option<Geometry>,
) -> Result<impl warp::Reply, Rejection> {
    let doc_id = params.doc_id.clone();
    let doc_type = params.doc_type.clone();
    let q = params.forward_geocoder_query.q.clone();
    let lang = params.forward_geocoder_query.lang.clone();

    let filters =
        filters::Filters::from((params.forward_geocoder_query, geometry, params.proximity));
    let dsl = dsl::build_query(
        &ctx.settings.elasticsearch.index_root,
        &q,
        &filters,
        lang.as_str(),
        &ctx.settings.query,
        QueryType::PREFIX,
        None,
        false,
    );

    match ctx
        .client
        .explain_search(Query::QueryDSL(dsl), doc_id, doc_type)
        .await
    {
        Ok(res) => Ok(with_status(json(&res), StatusCode::OK)),
        Err(err) => Err(warp::reject::custom(InternalError {
            reason: InternalErrorReason::ElasticSearchError,
            info: err.to_string(),
        })),
    }
}

pub async fn reverse_geocoder(
    ctx: Context,
    params: ReverseGeocoderQuery,
) -> Result<impl warp::Reply, Rejection> {
    let timeout = params.timeout.unwrap_or(ctx.settings.autocomplete_timeout);
    let distance = format!("{}m", ctx.settings.query.reverse_query.radius);
    let dsl = dsl::build_reverse_query(&distance, params.lat, params.lon);

    let es_indices_to_search_in = vec![
        root_doctype(
            &ctx.settings.elasticsearch.index_root,
            Street::static_doc_type(),
        ),
        root_doctype(
            &ctx.settings.elasticsearch.index_root,
            Addr::static_doc_type(),
        ),
    ];

    tracing::trace!(
        "Searching in indexes {:?} with query {}",
        es_indices_to_search_in,
        serde_json::to_string_pretty(&dsl).unwrap()
    );

    let places = ctx
        .client
        .search_documents(
            es_indices_to_search_in,
            Query::QueryDSL(dsl),
            params.limit,
            Some(timeout),
        )
        .await
        .map_err(|err| {
            warp::reject::custom(InternalError {
                reason: InternalErrorReason::ElasticSearchError,
                info: err.to_string(),
            })
        })?;

    let resp = GeocodeJsonResponse::from_with_lang(places, None);
    Ok(with_status(json(&resp), StatusCode::OK))
}

pub async fn status(ctx: Context) -> Result<impl warp::Reply, Rejection> {
    match ctx.client.status().await {
        Ok(res) => {
            let resp = StatusResponseBody {
                bragi: BragiStatus {
                    version: VERSION.to_string(),
                },
                mimir: MimirStatus {
                    version: res.version,
                },
                elasticsearch: ElasticsearchStatus {
                    version: res.storage.version,
                    health: res.storage.health.to_string(),
                    url: ctx.settings.elasticsearch.url.to_string(),
                },
            };
            Ok(with_status(json(&resp), StatusCode::OK))
        }
        Err(err) => Err(warp::reject::custom(InternalError {
            reason: InternalErrorReason::StatusError,
            info: err.to_string(),
        })),
    }
}

pub async fn metrics() -> Result<impl warp::Reply, Rejection> {
    let reply = warp::reply::with_header(
        prometheus_handler::metrics(),
        "content-type",
        "text/plain; charset=utf-8",
    );
    Ok(reply)
}
