use geojson::Geometry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::admin::Admin;
use super::context::Context;
use super::coord::Coord;
use super::i18n_properties::I18nProperties;
use super::{Members, MimirObject, PlaceDocType, Property};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Poi {
    pub id: String,
    pub label: String,
    pub name: String,
    pub coord: Coord,
    /// coord used for some geograhic queries in ES, less precise but  faster than `coord`
    /// https://www.elastic.co/guide/en/elasticsearch/reference/2.4/geo-shape.html
    #[serde(skip_deserializing)]
    pub approx_coord: Option<Geometry>,
    pub administrative_regions: Vec<Arc<Admin>>,
    pub weight: f64,
    pub zip_codes: Vec<String>,
    pub poi_type: PoiType,
    pub properties: Vec<Property>,
    // pub address: Option<Address>,
    pub address: Option<super::addr::Addr>,
    #[serde(default)]
    pub country_codes: Vec<String>,

    #[serde(default)]
    pub names: I18nProperties,
    #[serde(default)]
    pub labels: I18nProperties,

    /// Distance to the coord in query.
    /// Not serialized as is because it is returned in the `Feature` object
    #[serde(default, skip)]
    pub distance: Option<u32>,

    pub context: Option<Context>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PoiType {
    pub id: String,
    pub name: String,
}

impl From<&navitia_poi_model::PoiType> for PoiType {
    fn from(poi_type: &navitia_poi_model::PoiType) -> PoiType {
        PoiType {
            id: poi_type.id.clone(),
            name: poi_type.name.clone(),
        }
    }
}

impl MimirObject for Poi {
    fn is_geo_data() -> bool {
        true
    }
    fn doc_type() -> &'static str {
        PlaceDocType::Poi.as_str()
    }
    fn es_id(&self) -> Option<String> {
        Some(self.id.clone())
    }
}

impl Members for Poi {
    fn label(&self) -> &str {
        &self.label
    }
    fn admins(&self) -> Vec<Arc<Admin>> {
        self.administrative_regions.clone()
    }
}
