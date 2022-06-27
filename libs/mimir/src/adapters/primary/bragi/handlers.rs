use std::time::Duration;

use geo::algorithm::haversine_distance::HaversineDistance;
use geojson::Geometry;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use warp::{
    http::StatusCode,
    reject::Reject,
    reply::{json, with_status},
};

use crate::{
    adapters::{
        primary::{
            bragi::{
                api::{
                    BragiStatus, ElasticsearchStatus, ForwardGeocoderExplainQuery,
                    ForwardGeocoderQuery, MimirStatus, ReverseGeocoderQuery, StatusResponseBody,
                    Type,
                },
                prometheus_handler,
            },
            common::{
                coord, dsl,
                dsl::QueryType,
                filters,
                geocoding::{Feature, FromWithLang, GeocodeJsonResponse},
                settings::QuerySettings,
            },
        },
        secondary::elasticsearch::ElasticsearchStorageConfig,
    },
    domain::{
        model::{
            configuration::{root_doctype, root_doctype_dataset},
            query::Query,
        },
        ports::primary::{
            explain_query::ExplainDocument, search_documents::SearchDocuments, status::Status,
        },
    },
    utils::deserialize::deserialize_duration,
};
use common::document::ContainerDocument;
use places::{addr::Addr, admin::Admin, poi::Poi, stop::Stop, street::Street, Place};

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
pub struct Context<C> {
    pub client: C,
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
    places: Vec<places::Place>,
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
pub async fn forward_geocoder<C>(
    ctx: Context<C>,
    params: ForwardGeocoderQuery,
    geometry: Option<Geometry>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    C: SearchDocuments,
{
    let q = params.q.clone();
    let timeout = params.timeout.unwrap_or(ctx.settings.autocomplete_timeout);
    let es_indices_to_search_in =
        build_es_indices_to_search(&params.types, &params.pt_dataset, &params.poi_dataset);
    let lang = params.lang.clone();
    let filters = filters::Filters::from((params, geometry));
    let excludes = ["boundary".to_string()];

    for query_type in [QueryType::PREFIX, QueryType::FUZZY] {
        let dsl_query = dsl::build_query(
            &q,
            &filters,
            lang.as_str(),
            &ctx.settings.query,
            query_type,
            Option::Some(&excludes),
        );

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
                filters.limit,
                Some(timeout),
            )
            .await;

        #[cfg(feature = "metrics")]
        if let Some(timer) = timer {
            timer.observe_duration();
        }

        let places: Vec<Place> = res.map_err(|err| {
            warp::reject::custom(InternalError {
                reason: InternalErrorReason::ElasticSearchError,
                info: err.to_string(),
            })
        })?;

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
pub async fn forward_geocoder_explain<C>(
    ctx: Context<C>,
    params: ForwardGeocoderExplainQuery,
    geometry: Option<Geometry>,
) -> Result<impl warp::Reply, warp::Rejection>
where
    C: ExplainDocument,
    C::Document: Serialize + Into<serde_json::Value>,
{
    let doc_id = params.doc_id.clone();
    let doc_type = params.doc_type.clone();
    let q = params.q.clone();
    let lang = params.lang.clone();

    let filters = filters::Filters::from((params.into(), geometry));
    let dsl = dsl::build_query(
        &q,
        &filters,
        lang.as_str(),
        &ctx.settings.query,
        QueryType::PREFIX,
        None,
    );

    match ctx
        .client
        .explain_document(Query::QueryDSL(dsl), doc_id, doc_type)
        .await
    {
        Ok(res) => Ok(with_status(json(&res), StatusCode::OK)),
        Err(err) => Err(warp::reject::custom(InternalError {
            reason: InternalErrorReason::ElasticSearchError,
            info: err.to_string(),
        })),
    }
}

pub async fn reverse_geocoder<C>(
    ctx: Context<C>,
    params: ReverseGeocoderQuery,
) -> Result<impl warp::Reply, warp::Rejection>
where
    C: SearchDocuments,
{
    let timeout = params.timeout.unwrap_or(ctx.settings.autocomplete_timeout);
    let distance = format!("{}m", ctx.settings.query.reverse_query.radius);
    let dsl = dsl::build_reverse_query(&distance, params.lat, params.lon);

    let es_indices_to_search_in = vec![
        root_doctype(Street::static_doc_type()),
        root_doctype(Addr::static_doc_type()),
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

pub async fn status<C>(ctx: Context<C>) -> Result<impl warp::Reply, warp::Rejection>
where
    C: Status,
{
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

pub async fn metrics() -> Result<impl warp::Reply, warp::Rejection> {
    let reply = warp::reply::with_header(
        prometheus_handler::metrics(),
        "content-type",
        "text/plain; charset=utf-8",
    );
    Ok(reply)
}

pub fn build_es_indices_to_search(
    types: &Option<Vec<Type>>,
    pt_dataset: &Option<Vec<String>>,
    poi_dataset: &Option<Vec<String>>,
) -> Vec<String> {
    // some specific types are requested,
    // let's search only for these types of objects
    if let Some(types) = types {
        let mut indices = Vec::new();
        for doc_type in types.iter() {
            match doc_type {
                Type::House => indices.push(root_doctype(Addr::static_doc_type())),
                Type::Street => indices.push(root_doctype(Street::static_doc_type())),
                Type::Zone | Type::City => indices.push(root_doctype(Admin::static_doc_type())),
                Type::Poi => {
                    let doc_type_str = Poi::static_doc_type();
                    // if some poi_dataset are specified
                    // we search for poi only in the corresponding es indices
                    if let Some(poi_datasets) = poi_dataset {
                        for poi_dataset in poi_datasets.iter() {
                            indices.push(root_doctype_dataset(doc_type_str, poi_dataset));
                        }
                    } else {
                        // no poi_dataset specified
                        // we search in the global alias for all poi
                        indices.push(root_doctype(doc_type_str));
                    }
                }
                Type::StopArea => {
                    // if some pt_dataset are specified
                    // we search for stops only in the corresponding es indices
                    let doc_type_str = Stop::static_doc_type();
                    if let Some(pt_datasets) = pt_dataset {
                        for pt_dataset in pt_datasets.iter() {
                            indices.push(root_doctype_dataset(doc_type_str, pt_dataset));
                        }
                    } else {
                        // no pt_dataset specified
                        // we search in the global alias for all stops
                        indices.push(root_doctype(doc_type_str));
                    }
                }
            }
        }
        indices
    } else {
        let mut indices = vec![
            root_doctype(Addr::static_doc_type()),
            root_doctype(Street::static_doc_type()),
            root_doctype(Admin::static_doc_type()),
        ];
        if let Some(pt_datasets) = pt_dataset {
            let doc_type_str = Stop::static_doc_type();
            for pt_dataset in pt_datasets.iter() {
                indices.push(root_doctype_dataset(doc_type_str, pt_dataset));
            }
        }
        if let Some(poi_datasets) = poi_dataset {
            let doc_type_str = Poi::static_doc_type();
            for poi_dataset in poi_datasets.iter() {
                indices.push(root_doctype_dataset(doc_type_str, poi_dataset));
            }
        } else {
            indices.push(root_doctype(Poi::static_doc_type()))
        }
        indices
    }
}
