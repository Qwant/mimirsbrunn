use crate::coord::Coord;
use crate::filters::{Filters, HotelFilter, Proximity, Type};
use cosmogony::ZoneType;
use geojson::{GeoJson, Geometry};
use places::PlaceDocType;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;

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
    #[serde(
        deserialize_with = "serde_helpers::deserialize_f32",
        default = "serde_helpers::default_lat_lon"
    )]
    pub lat: Option<f32>,
    #[serde(
        deserialize_with = "serde_helpers::deserialize_f32",
        default = "serde_helpers::default_lat_lon"
    )]
    pub lon: Option<f32>,
    pub shape_scope: Option<Vec<PlaceDocType>>,
    #[serde(default, rename = "type")]
    pub types: Option<Vec<Type>>,
    #[serde(default, rename = "zone_type")]
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
    pub pt_dataset: Option<Vec<String>>,
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

/// This structure contains all the query parameters that
/// can be submitted for the reverse endpoint.
#[derive(Debug, Default, Serialize, Deserialize)]
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