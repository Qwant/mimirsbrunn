use std::time::Duration;

use crate::coord::Coord;
use geojson::Geometry;
use places::addr::Addr;
use places::admin::Admin;
use places::poi::Poi;
use places::stop::Stop;
use places::street::Street;
use places::ContainerDocument;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

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

#[derive(PartialEq, Copy, Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HotelFilter {
    Exclude,
    #[default]
    No,
    Yes,
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

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Proximity {
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "proximity_scale")]
    pub scale: f64,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "proximity_offset")]
    pub offset: f64,
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "proximity_decay")]
    pub decay: f64,
}
