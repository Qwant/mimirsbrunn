use schemars::JsonSchema;
use std::time::Duration;

use places::addr::Addr;
use places::admin::Admin;
use places::coord::Coord;
use places::poi::Poi;
use places::street::Street;
use places::ContainerDocument;
use qwant_geojson::Geometry;
use serde::{Deserialize, Serialize};

// How to restrict the range of the query... Except for the place type (ie what indices we're
// searching, since we use the list of types to create the list of indices to search for just
// before calling search_documents.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub coord: Option<Coord>,
    pub shape: Option<(Geometry, Vec<String>)>, // We use String rather than Type to avoid dependencies toward bragi api.
    pub zone_types: Option<Vec<String>>,
    pub poi_types: Option<Vec<String>>,
    pub limit: i64,
    pub timeout: Option<Duration>,
    pub proximity: Option<Proximity>,
    pub is_hotel_filter: HotelFilter,
    pub is_famous_poi: bool,
}

#[derive(PartialEq, Copy, Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HotelFilter {
    Exclude,
    #[default]
    No,
    Yes,
}

#[derive(PartialEq, Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    #[serde(rename = "house")]
    House,
    #[serde(rename = "poi")]
    Poi,
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
            Type::Street => "street",
            Type::Zone => "zone",
            Type::City => "city",
        }
    }

    pub fn as_index_type(&self) -> &'static str {
        match self {
            Type::House => Addr::static_doc_type(),
            Type::Poi => Poi::static_doc_type(),
            Type::Street => Street::static_doc_type(),
            Type::Zone | Type::City => Admin::static_doc_type(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Proximity {
    pub proximity_scale: f64,
    pub proximity_offset: f64,
    pub proximity_decay: f64,
}
