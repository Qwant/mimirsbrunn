use crate::filters::{Filters, HotelFilter, Proximity, Type};
use cosmogony::ZoneType;
use places::admin::ZoneTypeDef;
use places::coord::Coord;
use places::PlaceDocType;
use qwant_geojson::{GeoJson, Geometry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;
use validator::Validate;

#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
pub struct ForwardGeocoderExplainQuery {
    pub doc_id: String,
    pub doc_type: String,
    #[serde(flatten)]
    pub forward_geocoder_query: GeocoderQuery,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
}
/// This structure contains all the query parameters that
/// can be submitted for the autocomplete endpoint.
///
/// Only the `q` parameter is mandatory.
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct GeocoderQuery {
    #[serde(default)]
    #[validate(length(min = 1))]
    pub q: String,
    #[validate(range(min = - 90, max = 90))]
    pub lat: Option<f32>,
    #[validate(range(min = - 180, max = 180))]
    pub lon: Option<f32>,
    pub shape_scope: Option<Vec<PlaceDocType>>,
    #[serde(default, rename = "type[]", alias = "type")]
    pub types: Vec<Type>,
    #[serde(default, rename = "zone_type")]
    #[schemars(with = "ZoneTypeDef")]
    pub zone_types: Option<Vec<ZoneType>>,
    pub poi_types: Option<Vec<String>>,
    #[serde(
        deserialize_with = "serde_helpers::deserialize_i64",
        default = "serde_helpers::default_result_limit"
    )]
    pub limit: i64,
    #[serde(default = "serde_helpers::default_lang")]
    pub lang: String,
    #[serde(deserialize_with = "serde_helpers::deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
    pub poi_dataset: Option<Vec<String>>,
    pub request_id: Option<String>,
    #[serde(
        deserialize_with = "serde_helpers::deserialize_bool",
        default = "serde_helpers::default_false"
    )]
    pub is_exact_match: bool,
    pub is_hotel_filter: HotelFilter,
    #[serde(
        deserialize_with = "serde_helpers::deserialize_bool",
        default = "serde_helpers::default_false"
    )]
    pub is_famous_poi: bool,
    #[serde(flatten)]
    pub proximity: Option<Proximity>,
    #[serde(flatten)]
    pub geometry: Option<Geometry>,
}

impl From<GeocoderQuery> for Filters {
    fn from(query: GeocoderQuery) -> Self {
        let zone_types = query
            .zone_types
            .map(|zts| zts.iter().map(|t| t.as_str().to_string()).collect());

        Filters {
            // When option_zip_option becomes available: coord: input.lat.zip_with(input.lon, Coord::new),
            coord: match (query.lat, query.lon) {
                (Some(lat), Some(lon)) => Some(Coord::new(lat, lon)),
                _ => None,
            },
            shape: query.geometry.map(|geometry| {
                (
                    geometry,
                    query
                        .shape_scope
                        .map(|shape_scope| {
                            shape_scope.iter().map(|t| t.as_str().to_string()).collect()
                        })
                        .unwrap_or_else(|| {
                            [
                                PlaceDocType::Poi,
                                PlaceDocType::Street,
                                PlaceDocType::Admin,
                                PlaceDocType::Addr,
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
            is_hotel_filter: query.is_hotel_filter,
            is_famous_poi: query.is_famous_poi,
        }
    }
}

/// This structure contains all the query parameters that
/// can be submitted for the reverse endpoint.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "snake_case")]
pub struct ReverseGeocoderQuery {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "serde_helpers::default_result_limit_reverse")]
    pub limit: i64,
    #[serde(deserialize_with = "serde_helpers::deserialize_opt_duration", default)]
    pub timeout: Option<Duration>,
}

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
