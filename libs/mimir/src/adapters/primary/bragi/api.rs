use crate::{ensure, utils::deserialize::deserialize_opt_duration};
use cosmogony::ZoneType;
use geojson::{GeoJson, Geometry};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;

use crate::adapters::primary::common::{coord::Coord, filters::Filters};
use common::document::ContainerDocument;
use places::{addr::Addr, admin::Admin, poi::Poi, stop::Stop, street::Street, PlaceDocType};

use super::routes::{is_valid_zone_type, Validate};

pub const DEFAULT_LIMIT_RESULT_ES: i64 = 10;
pub const DEFAULT_LIMIT_RESULT_REVERSE_API: i64 = 1;
pub const DEFAULT_LANG: &str = "fr";

fn default_result_limit() -> i64 {
    DEFAULT_LIMIT_RESULT_ES
}

fn default_result_limit_reverse() -> i64 {
    DEFAULT_LIMIT_RESULT_REVERSE_API
}

fn default_lang() -> String {
    DEFAULT_LANG.to_string()
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForwardGeocoderExplainQuery {
    pub doc_id: String,
    pub doc_type: String,

    // Fields from ForwardGeocoderQuery are repeated here as nesting two levels
    // of `flatten` with serde_qs is not supported.
    // See https://github.com/samscott89/serde_qs/issues/14
    pub q: String,
    pub lat: Option<f32>,
    pub lon: Option<f32>,
    pub shape_scope: Option<Vec<PlaceDocType>>,
    #[serde(default, rename = "type")]
    pub types: Option<Vec<Type>>,
    #[serde(default, rename = "zone_type")]
    pub zone_types: Option<Vec<ZoneType>>,
    pub poi_types: Option<Vec<String>>,
    #[serde(default = "default_result_limit")]
    pub limit: i64,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(deserialize_with = "deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
    pub pt_dataset: Option<Vec<String>>,
    pub poi_dataset: Option<Vec<String>>,
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
}

impl From<ForwardGeocoderExplainQuery> for ForwardGeocoderQuery {
    fn from(val: ForwardGeocoderExplainQuery) -> Self {
        let ForwardGeocoderExplainQuery {
            q,
            lat,
            lon,
            shape_scope,
            types,
            zone_types,
            poi_types,
            limit,
            lang,
            timeout,
            pt_dataset,
            poi_dataset,
            request_id,
            proximity,
            ..
        } = val;

        ForwardGeocoderQuery {
            q,
            lat,
            lon,
            shape_scope,
            types,
            zone_types,
            poi_types,
            limit,
            lang,
            timeout,
            pt_dataset,
            poi_dataset,
            request_id,
            proximity,
        }
    }
}

impl Validate for ForwardGeocoderExplainQuery {
    fn filter(&self) -> Result<(), warp::Rejection> {
        ensure! {
            !self.q.is_empty();

            self.lat.is_some() == self.lon.is_some(),
                "lat and lon parameters must both be specified";

            self.lat.map(|lat| (-90f32..=90f32).contains(&lat)).unwrap_or(true),
                "lat must be in [-90, 90]";

            self.lon.map(|lon| (-180f32..=180f32).contains(&lon)).unwrap_or(true),
                "lon must be in [-180, 180]";
        }
    }
}

/// This structure contains all the query parameters that
/// can be submitted for the autocomplete endpoint.
///
/// Only the `q` parameter is mandatory.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForwardGeocoderQuery {
    pub q: String,
    pub lat: Option<f32>,
    pub lon: Option<f32>,
    pub shape_scope: Option<Vec<PlaceDocType>>,
    #[serde(default, rename = "type")]
    pub types: Option<Vec<Type>>,
    #[serde(default, rename = "zone_type")]
    pub zone_types: Option<Vec<ZoneType>>,
    pub poi_types: Option<Vec<String>>,
    #[serde(default = "default_result_limit")]
    pub limit: i64,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(deserialize_with = "deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
    pub pt_dataset: Option<Vec<String>>,
    pub poi_dataset: Option<Vec<String>>,
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
}

impl From<(ForwardGeocoderQuery, Option<Geometry>)> for Filters {
    fn from(source: (ForwardGeocoderQuery, Option<Geometry>)) -> Self {
        let (query, geometry) = source;
        let zone_types = query
            .zone_types
            .map(|zts| zts.iter().map(|t| t.as_str().to_string()).collect());

        Filters {
            // When option_zip_option becomes available: coord: input.lat.zip_with(input.lon, Coord::new),
            coord: match (query.lat, query.lon) {
                (Some(lat), Some(lon)) => Some(Coord::new(lat, lon)),
                _ => None,
            },
            shape: geometry.map(|geometry| {
                (
                    geometry,
                    query
                        .shape_scope
                        .map(|shape_scope| {
                            shape_scope.iter().map(|t| t.as_str().to_string()).collect()
                        })
                        .unwrap_or_else(|| {
                            vec![
                                PlaceDocType::Poi,
                                PlaceDocType::Street,
                                PlaceDocType::Admin,
                                PlaceDocType::Addr,
                                PlaceDocType::Stop,
                            ]
                            .iter()
                            .map(|t| t.as_str().to_string())
                            .collect()
                        }),
                )
            }),
            zone_types,
            poi_types: query.poi_types,
            limit: query.limit,
            timeout: query.timeout,
            proximity: query.proximity,
        }
    }
}

impl Validate for ForwardGeocoderQuery {
    fn filter(&self) -> Result<(), warp::Rejection> {
        ensure! {
            !self.q.is_empty();

            self.lat.is_some() == self.lon.is_some(),
                "lat and lon parameters must both be specified";

            self.lat.map(|lat| (-90f32..=90f32).contains(&lat)).unwrap_or(true),
                "lat must be in [-90, 90]";

            self.lon.map(|lon| (-180f32..=180f32).contains(&lon)).unwrap_or(true),
                "lon must be in [-180, 180]";

            is_valid_zone_type(self),
                "'zone_type' must be specified when you query with 'type' parameter 'zone'";
        }
    }
}

/// This structure contains all the query parameters that
/// can be submitted for the reverse endpoint.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReverseGeocoderQuery {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_result_limit_reverse")]
    pub limit: i64,
    #[serde(deserialize_with = "deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
}

impl Validate for ReverseGeocoderQuery {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ForwardGeocoderBody {
    pub shape: GeoJson,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExplainResponseBody {
    pub explanation: JsonValue,
}

impl From<JsonValue> for ExplainResponseBody {
    fn from(explanation: JsonValue) -> Self {
        ExplainResponseBody { explanation }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BragiStatus {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MimirStatus {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ElasticsearchStatus {
    pub version: String,
    pub health: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StatusResponseBody {
    pub bragi: BragiStatus,
    pub mimir: MimirStatus,
    pub elasticsearch: ElasticsearchStatus,
}

#[derive(PartialEq, Copy, Clone, Debug, Deserialize, Serialize)]
pub enum Type {
    #[serde(rename = "house")]
    House,
    #[serde(rename = "poi")]
    Poi,
    #[serde(rename = "public_transport:stop_area")]
    StopArea,
    #[serde(rename = "street")]
    Street,
    #[serde(rename = "zone")]
    Zone,
    // TODO To be deleted when switching to full ES7 (in production)
    #[serde(rename = "city")]
    City,
}

impl Type {
    pub fn as_str(&self) -> &'static str {
        match self {
            Type::House => "house",
            Type::Poi => "poi",
            Type::StopArea => "public_transport:stop_area",
            Type::Street => "street",
            Type::Zone => "zone",
            Type::City => "city",
        }
    }

    pub fn as_index_type(&self) -> &'static str {
        match self {
            Type::House => Addr::static_doc_type(),
            Type::Poi => Poi::static_doc_type(),
            Type::StopArea => Stop::static_doc_type(),
            Type::Street => Street::static_doc_type(),
            Type::Zone | Type::City => Admin::static_doc_type(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Proximity {
    #[serde(with = "serde_with::rust::display_fromstr")]
    #[serde(rename = "proximity_scale")]
    pub scale: f64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    #[serde(rename = "proximity_offset")]
    pub offset: f64,
    #[serde(with = "serde_with::rust::display_fromstr")]
    #[serde(rename = "proximity_decay")]
    pub decay: f64,
}
