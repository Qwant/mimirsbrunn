use crate::ensure;
use elastic_query_builder::query::{
    ForwardGeocoderExplainQuery, ForwardGeocoderParamsQuery, ReverseGeocoderQuery,
};
use serde::{Deserialize, Serialize};

use crate::routes::{is_valid_zone_type, Validate};

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

impl Validate for ReverseGeocoderQuery {}
