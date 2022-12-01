use common::document::{ContainerDocument, Document};
use cosmogony::ZoneType;
use geo_types::{MultiPolygon, Rect};
use geojson::Geometry;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap, sync::Arc};

use super::{
    context::Context,
    coord::Coord,
    i18n_properties::I18nProperties,
    utils::{
        custom_multi_polygon_deserialize, custom_multi_polygon_serialize, deserialize_rect,
        get_country_code, serialize_rect,
    },
    Members,
};

pub const ADMIN_MAX_WEIGHT: f64 = 1_400_000_000.; // China's population

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(tag = "type", rename = "admin")]
pub struct Admin {
    pub id: String,
    pub insee: String,
    pub level: u32,
    pub label: String,
    pub name: String,
    pub alt_name: Option<String>,
    pub loc_name: Option<String>,
    pub zip_codes: Vec<String>,
    pub weight: f64,
    /// coord used for some geograhic queries in ES, less precise but  faster than `coord`
    /// https://www.elastic.co/guide/en/elasticsearch/reference/2.4/geo-shape.html
    #[serde(skip_deserializing)]
    pub approx_coord: Option<Geometry>,
    pub coord: Coord,
    #[serde(
        serialize_with = "custom_multi_polygon_serialize",
        deserialize_with = "custom_multi_polygon_deserialize",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub boundary: Option<MultiPolygon<f64>>,
    #[serde(default)]
    pub administrative_regions: Vec<Arc<Admin>>,

    #[serde(
        serialize_with = "serialize_rect",
        deserialize_with = "deserialize_rect",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub bbox: Option<Rect<f64>>,

    #[serde(default)]
    pub zone_type: Option<ZoneType>,
    #[serde(default)]
    pub parent_id: Option<String>, // id of the Admin's parent (from the cosmogony's hierarchy)
    #[serde(default)]
    pub country_codes: Vec<String>,

    #[serde(default)]
    pub codes: BTreeMap<String, String>,

    #[serde(default)]
    pub names: I18nProperties,

    #[serde(default)]
    pub alt_names: I18nProperties,

    #[serde(default)]
    pub loc_names: I18nProperties,

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

pub fn get_zip_codes_from_admins(admins: &[Arc<Admin>]) -> Vec<String> {
    let level = admins.iter().fold(0, |level, adm| {
        if adm.level > level && !adm.zip_codes.is_empty() {
            adm.level
        } else {
            level
        }
    });
    if level == 0 {
        return vec![];
    }
    admins
        .iter()
        .filter(|adm| adm.level == level)
        .flat_map(|adm| adm.zip_codes.iter().cloned())
        .collect()
}

/// normalize the admin weight for it to be in [0, 1]
pub fn normalize_admin_weight(admins: &mut [Admin]) {
    for admin in admins {
        admin.weight = normalize_weight(admin.weight, ADMIN_MAX_WEIGHT);
    }
}

/// normalize the weight for it to be in [0, 1]
pub fn normalize_weight(weight: f64, max_weight: f64) -> f64 {
    (weight / max_weight).clamp(0., 1.)
}

pub fn find_country_codes<'a>(admins: impl Iterator<Item = &'a Admin>) -> Vec<String> {
    admins.filter_map(|a| get_country_code(&a.codes)).collect()
}

impl Admin {
    pub fn is_city(&self) -> bool {
        matches!(self.zone_type, Some(ZoneType::City))
    }
}

impl Document for Admin {
    fn id(&self) -> String {
        self.id.clone()
    }
}

impl ContainerDocument for Admin {
    fn static_doc_type() -> &'static str {
        "admin"
    }
}

impl Ord for Admin {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Admin {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Admin {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Members for Admin {
    fn label(&self) -> &str {
        &self.label
    }
    fn admins(&self) -> Vec<Arc<Admin>> {
        vec![Arc::new(self.clone())]
    }
}

impl Eq for Admin {}

impl From<&Admin> for geojson::Geometry {
    fn from(admin: &Admin) -> Self {
        geojson::Geometry::from(admin.coord)
    }
}
