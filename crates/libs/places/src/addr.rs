use super::{ContainerDocument, Document};
use qwant_geojson::Geometry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::admin::Admin;
use super::context::Context;
use super::coord::Coord;
use super::street::Street;
use super::Members;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename = "addr")]
pub struct Addr {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub house_number: String,
    pub street: Street,
    pub label: String,
    pub coord: Coord,
    /// coord used for some geograhic queries in ES, less precise but  faster than `coord`
    /// https://www.elastic.co/guide/en/elasticsearch/reference/2.4/geo-shape.html
    #[serde(skip_deserializing)]
    pub approx_coord: Option<Geometry>,
    pub weight: f64,
    pub zip_codes: Vec<String>,
    #[serde(default)]
    pub country_codes: Vec<String>,
    /// Distance to the coord in query.
    /// Not serialized as is because it is returned in the `Feature` object
    #[serde(default, skip)]
    pub distance: Option<u32>,

    pub context: Option<Context>,
}

impl Members for Addr {
    fn label(&self) -> &str {
        &self.label
    }
    fn admins(&self) -> Vec<Arc<Admin>> {
        self.street.admins()
    }
}

impl Document for Addr {
    fn id(&self) -> String {
        self.id.clone()
    }
}

impl Addr {
    pub fn city(&self) -> Option<String> {
        self.street
            .administrative_regions
            .iter()
            .find(|&admin| admin.is_city())
            .map(|admin| admin.name.clone())
    }
}

impl ContainerDocument for Addr {
    fn static_doc_type() -> &'static str {
        "addr"
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AliasOperations {
    pub actions: Vec<AliasOperation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AliasOperation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<AliasParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove: Option<AliasParameter>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AliasParameter {
    pub index: String,
    pub alias: String,
}

impl From<&Addr> for Geometry {
    fn from(addr: &Addr) -> Self {
        Geometry::Point(vec![addr.coord.lon(), addr.coord.lat()])
    }
}
