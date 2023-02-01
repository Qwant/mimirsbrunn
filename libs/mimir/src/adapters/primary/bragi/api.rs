use crate::{ensure, utils::deserialize::deserialize_opt_duration};
use cosmogony::ZoneType;
use geojson::{GeoJson, Geometry};
use serde::{de, Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;

use crate::adapters::primary::bragi::api::HotelFilter::No;
use crate::adapters::primary::common::{coord::Coord, filters::Filters};
use common::document::ContainerDocument;
use places::{addr::Addr, admin::Admin, poi::Poi, stop::Stop, street::Street, PlaceDocType};

use super::routes::{is_valid_zone_type, Validate};

pub const DEFAULT_LIMIT_RESULT_ES: i64 = 10;
pub const DEFAULT_LIMIT_RESULT_REVERSE_API: i64 = 1;
pub const DEFAULT_LANG: &str = "fr";

#[derive(PartialEq, Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HotelFilter {
    Exclude,
    No,
    Yes,
}

impl Default for HotelFilter {
    fn default() -> Self {
        No
    }
}

fn default_result_limit() -> i64 {
    DEFAULT_LIMIT_RESULT_ES
}

fn default_lat_lon() -> Option<f32> {
    None
}

fn default_result_limit_reverse() -> i64 {
    DEFAULT_LIMIT_RESULT_REVERSE_API
}

fn default_false() -> bool {
    false
}

fn default_lang() -> String {
    DEFAULT_LANG.to_string()
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForwardGeocoderExplainQuery {
    pub doc_id: String,
    pub doc_type: String,
    #[serde(flatten)]
    pub forward_geocoder_query: ForwardGeocoderQuery,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForwardGeocoderParamsQuery {
    #[serde(flatten)]
    pub forward_geocoder_query: ForwardGeocoderQuery,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
}

/// This structure contains all the query parameters that
/// can be submitted for the autocomplete endpoint.
///
/// Only the `q` parameter is mandatory.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForwardGeocoderQuery {
    #[serde(default)]
    pub q: String,
    // Use of deserialize_with within flatten struct because the lib doesn't deserializing correctly
    #[serde(deserialize_with = "deserialize_f32", default = "default_lat_lon")]
    pub lat: Option<f32>,
    #[serde(deserialize_with = "deserialize_f32", default = "default_lat_lon")]
    pub lon: Option<f32>,
    pub shape_scope: Option<Vec<PlaceDocType>>,
    #[serde(default, rename = "type")]
    pub types: Option<Vec<Type>>,
    #[serde(default, rename = "zone_type")]
    pub zone_types: Option<Vec<ZoneType>>,
    pub poi_types: Option<Vec<String>>,
    #[serde(deserialize_with = "deserialize_i64", default = "default_result_limit")]
    pub limit: i64,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(deserialize_with = "deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
    pub pt_dataset: Option<Vec<String>>,
    pub poi_dataset: Option<Vec<String>>,
    pub request_id: Option<String>,
    #[serde(deserialize_with = "deserialize_bool", default = "default_false")]
    pub is_exact_match: bool,
    pub is_hotel_filter: HotelFilter,
    #[serde(deserialize_with = "deserialize_bool", default = "default_false")]
    pub is_famous_poi: bool,
}

fn deserialize_f32<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    Ok(Some(s.parse::<f32>().unwrap()))
}

fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    Ok(s.parse::<i64>().unwrap())
}

fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;

    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Ok(false),
    }
}

impl From<(ForwardGeocoderQuery, Option<Geometry>, Option<Proximity>)> for Filters {
    fn from(source: (ForwardGeocoderQuery, Option<Geometry>, Option<Proximity>)) -> Self {
        let (query, geometry, proximity) = source;
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
            proximity,
            is_hotel_filter: query.is_hotel_filter,
            is_famous_poi: query.is_famous_poi,
        }
    }
}

impl Validate for ForwardGeocoderParamsQuery {
    fn filter(&self) -> Result<(), warp::Rejection> {
        ensure! {
            !self.forward_geocoder_query.q.is_empty();

            self.forward_geocoder_query.lat.is_some() == self.forward_geocoder_query.lon.is_some(),
                "lat and lon parameters must either be both present or both absent";

            self.forward_geocoder_query.lat.map(|lat| (-90f32..=90f32).contains(&lat)).unwrap_or(true),
                "lat must be in [-90, 90]";

            self.forward_geocoder_query.lon.map(|lon| (-180f32..=180f32).contains(&lon)).unwrap_or(true),
                "lon must be in [-180, 180]";

            is_valid_zone_type(&self.forward_geocoder_query),
                "'zone_type' must be specified when you query with 'type' parameter 'zone'";
        }
    }
}

impl Validate for ForwardGeocoderExplainQuery {
    fn filter(&self) -> Result<(), warp::Rejection> {
        ensure! {
            !self.forward_geocoder_query.q.is_empty();

            self.forward_geocoder_query.lat.is_some() == self.forward_geocoder_query.lon.is_some(),
                "lat and lon parameters must either be both present or both absent";

            self.forward_geocoder_query.lat.map(|lat| (-90f32..=90f32).contains(&lat)).unwrap_or(true),
                "lat must be in [-90, 90]";

            self.forward_geocoder_query.lon.map(|lon| (-180f32..=180f32).contains(&lon)).unwrap_or(true),
                "lon must be in [-180, 180]";

            is_valid_zone_type(&self.forward_geocoder_query),
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
