use std::collections::BTreeMap;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use qwant_geojson::Geometry;

use crate::Address;

use super::admin::Admin;
use super::context::Context;
use super::coord::Coord;
use super::i18n_properties::I18nProperties;
use super::Members;
use super::{ContainerDocument, Document};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(tag = "type", rename = "poi")]
pub struct Poi {
    pub id: String,
    pub label: String,
    pub name: String,
    pub coord: Coord,
    /// coord used for some geograhic queries in ES, less precise but faster than `coord`
    /// https://www.elastic.co/guide/en/elasticsearch/reference/2.4/geo-shape.html
    #[serde(skip_deserializing)]
    pub approx_coord: Option<Geometry>,
    pub administrative_regions: Vec<Arc<Admin>>,
    pub weight: f64,
    pub zip_codes: Vec<String>,
    pub poi_type: PoiType,
    pub properties: BTreeMap<String, String>,
    pub address: Option<Address>,
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

    #[serde(default)]
    pub full_label_extra: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Default)]
pub struct PoiType {
    pub id: String,
    pub name: String,
}

impl Document for Poi {
    fn id(&self) -> String {
        self.id.clone()
    }
}

impl ContainerDocument for Poi {
    fn static_doc_type() -> &'static str {
        "poi"
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

impl From<&Poi> for Geometry {
    fn from(poi: &Poi) -> Self {
        Geometry::Point(vec![poi.coord.lon(), poi.coord.lat()])
    }
}
