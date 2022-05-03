use crate::adapters::primary::bragi::{
    api::{
        FeaturesQuery, ForwardGeocoderExplainQuery, ForwardGeocoderQuery, JsonParam,
        ReverseGeocoderQuery, Type,
    },
    handlers::{InternalError, InternalErrorReason},
};
use geojson::{GeoJson, Geometry};
use serde::{Deserialize, Serialize};
use serde_qs::Config;
use std::convert::Infallible;
use tracing::instrument;
use warp::{
    http::StatusCode,
    reject::{MethodNotAllowed, Reject},
    Filter, Rejection, Reply,
};

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiError {
    pub short: String,
    pub long: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum InvalidRequestReason {
    CannotDeserialize,
    EmptyQueryString,
    InconsistentPoiRequest,
    InconsistentZoneRequest,
    InconsistentLatLonRequest,
    OutOfRangeLatLonRequest,
}

#[derive(Deserialize, Serialize, Debug)]
struct InvalidRequest {
    pub reason: InvalidRequestReason,
    pub info: String,
}

impl Reject for InvalidRequest {}

#[derive(Debug)]
struct InvalidPostBody;
impl Reject for InvalidPostBody {}

/// Extract and Validate input parameters from the query
#[instrument]
pub fn forward_geocoder_query(
) -> impl Filter<Extract = (ForwardGeocoderQuery,), Error = Rejection> + Copy {
    warp::filters::query::raw()
        .and_then(|param: String| async move {
            // max_depth=1:
            // for more informations: https://docs.rs/serde_qs/latest/serde_qs/index.html
            let config = Config::new(2, false);
            tracing::info!("Autocomplete query : {}", param);
            config.deserialize_str(&param).map_err(|err| {
                warp::reject::custom(InvalidRequest {
                    reason: InvalidRequestReason::CannotDeserialize,
                    info: err.to_string(),
                })
            })
        })
        .and_then(ensure_query_string_not_empty)
        .and_then(ensure_zone_type_consistent)
        .and_then(ensure_lat_lon_consistent)
}

/// Extract and Validate input parameters from the query
#[instrument]
pub fn forward_geocoder_explain_query(
) -> impl Filter<Extract = (ForwardGeocoderExplainQuery,), Error = Rejection> + Copy {
    warp::filters::query::raw().and_then(|param: String| async move {
        // max_depth=1:
        // for more informations: https://docs.rs/serde_qs/latest/serde_qs/index.html
        let config = Config::new(2, false);
        tracing::info!("forward_geocoder_explain query : {}", param);
        config.deserialize_str(&param).map_err(|err| {
            warp::reject::custom(InvalidRequest {
                reason: InvalidRequestReason::CannotDeserialize,
                info: err.to_string(),
            })
        })
    })
}

pub async fn ensure_query_string_not_empty(
    params: ForwardGeocoderQuery,
) -> Result<ForwardGeocoderQuery, Rejection> {
    if params.q.is_empty() {
        Err(warp::reject::custom(InvalidRequest {
            reason: InvalidRequestReason::EmptyQueryString,
            info: "You must provide and non-empty query string".to_string(),
        }))
    } else {
        Ok(params)
    }
}

/// This filter ensures that if the user specifies lat or lon,
/// then he must specify also lon or lat.
pub async fn ensure_lat_lon_consistent(
    params: ForwardGeocoderQuery,
) -> Result<ForwardGeocoderQuery, Rejection> {
    match (params.lat, params.lon) {
        (Some(lat), Some(lon)) => {
            if !(-90f32..=90f32).contains(&lat) {
                Err(warp::reject::custom(InvalidRequest {
                    reason: InvalidRequestReason::OutOfRangeLatLonRequest,
                    info: format!("requested latitude {} is outside of range [-90;90]", lat),
                }))
            } else if !(-180f32..=180f32).contains(&lon) {
                Err(warp::reject::custom(InvalidRequest {
                    reason: InvalidRequestReason::OutOfRangeLatLonRequest,
                    info: format!("requested longitude {} is outside of range [-180;180]", lon),
                }))
            } else {
                Ok(params)
            }
        }
        (None, None) => Ok(params),
        (_, _) => Err(warp::reject::custom(InvalidRequest {
            reason: InvalidRequestReason::InconsistentLatLonRequest,
            info: "you should provide a 'lon' AND a 'lat' parameter if you provide one of them"
                .to_string(),
        })),
    }
}

/// This filter ensures that if the user requests 'zone', then he must specify the list
/// of zone_types.
pub async fn ensure_zone_type_consistent(
    params: ForwardGeocoderQuery,
) -> Result<ForwardGeocoderQuery, Rejection> {
    if params
        .types
        .as_ref()
        .map(|types| types.iter().any(|s| *s == Type::Zone))
        .unwrap_or(false)
        && params
            .zone_types
            .as_ref()
            .map(|zone_types| zone_types.is_empty())
            .unwrap_or(true)
    {
        Err(warp::reject::custom(InvalidRequest {
            reason: InvalidRequestReason::InconsistentZoneRequest,
            info: "'zone_type' must be specified when you query with 'type' parameter 'zone'"
                .to_string(),
        }))
    } else {
        Ok(params)
    }
}

// This filter extracts the GeoJson shape from the body of the request
#[instrument]
pub fn forward_geocoder_body(
) -> impl Filter<Extract = (Option<Geometry>,), Error = Rejection> + Copy {
    warp::body::content_length_limit(1024 * 32)
        .and(warp::body::json())
        .and_then(validate_geojson_shape)
}

pub async fn validate_geojson_shape(json: JsonParam) -> Result<Option<Geometry>, Rejection> {
    match json.shape {
        GeoJson::Feature(f) => f
            .geometry
            .ok_or_else(|| warp::reject::custom(InvalidPostBody))
            .map(Some),
        _ => Err(warp::reject::custom(InvalidPostBody)),
    }
}

pub fn reverse_geocoder_query(
) -> impl Filter<Extract = (ReverseGeocoderQuery,), Error = Rejection> + Copy {
    warp::filters::query::raw().and_then(|param: String| async move {
        let config = Config::new(2, false);
        tracing::info!("Reverse geocoder query : {}", param);
        config.deserialize_str(&param).map_err(|err| {
            warp::reject::custom(InvalidRequest {
                reason: InvalidRequestReason::CannotDeserialize,
                info: err.to_string(),
            })
        })
    })
}

pub fn features_query() -> impl Filter<Extract = (FeaturesQuery,), Error = Rejection> + Copy {
    warp::filters::query::raw().and_then(|param: String| async move {
        let config = Config::new(2, false);
        tracing::info!("Features query : {}", param);
        config.deserialize_str(&param).map_err(|err| {
            warp::reject::custom(InvalidRequest {
                reason: InvalidRequestReason::CannotDeserialize,
                info: err.to_string(),
            })
        })
    })
}

pub async fn report_invalid(rejection: Rejection) -> Result<impl Reply, Infallible> {
    let reply = if let Some(err) = rejection.find::<warp::reject::InvalidQuery>() {
        tracing::info!("Invalid query {:?}", err);
        warp::reply::with_status(
            warp::reply::json(&ApiError {
                short: "invalid query".to_string(),
                long: err.to_string(),
            }),
            StatusCode::BAD_REQUEST,
        )
    } else if let Some(err) = rejection.find::<InvalidRequest>() {
        tracing::info!("Invalid request {:?}", err);
        warp::reply::with_status(
            warp::reply::json(&ApiError {
                short: "validation error".to_string(),
                long: err.info.clone(),
            }),
            StatusCode::BAD_REQUEST,
        )
    } else if let Some(err) = rejection.find::<InternalError>() {
        tracing::info!("Internal error {:?}", err);
        let short = match err.reason {
            InternalErrorReason::ObjectNotFoundError => "Unable to find object".to_string(),
            _ => "query error".to_string(),
        };
        warp::reply::with_status(
            warp::reply::json(&ApiError {
                short,
                long: err.info.clone(),
            }),
            StatusCode::BAD_REQUEST,
        )
    } else if let Some(err) = rejection.find::<MethodNotAllowed>() {
        tracing::info!("MethodNotAllowed {:?}", err);
        warp::reply::with_status(
            warp::reply::json(&ApiError {
                short: "no route".to_string(),
                long: err.to_string(),
            }),
            StatusCode::NOT_FOUND,
        )
    } else {
        tracing::info!("Internal server error");
        warp::reply::with_status(
            warp::reply::json(&ApiError {
                short: "INTERNAL_SERVER_ERROR".to_string(),
                long: "INTERNAL_SERVER_ERROR".to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    };
    let reply = warp::reply::with_header(reply, "content-type", "application/json");
    Ok(reply)
}

pub fn cache_filter<F, T>(
    filter: F,
    http_cache_duration: usize,
) -> impl Filter<Extract = impl Reply, Error = std::convert::Infallible> + Clone + Send + Sync
where
    F: Filter<Extract = (T,), Error = std::convert::Infallible> + Clone + Send + Sync,
    F::Extract: warp::Reply,
    T: warp::Reply,
{
    warp::any().and(filter).map(move |reply| {
        warp::reply::with_header(
            reply,
            "cache-control",
            format!("max-age={}", http_cache_duration),
        )
    })
}
