use convert_case::{Case, Casing};
use serde::Serialize;
use std::sync::Arc;

use crate::filters::Type;
use places::admin::ZoneTypeDef;
use places::rect::Rect;
use schemars::JsonSchema;

/// GeocodeJSON is a an extension of the GeoJSON standard.
// It must contain the following three items
#[derive(Serialize, Debug, JsonSchema)]
pub struct GeocodeJsonResponse {
    /// Since GeocodeJSON must be valid GeoJSON, we must identify the type of object.
    /// We are returning a set of features, so the value of format_type will always be
    /// "FeatureCollection".
    #[serde(rename = "type")]
    pub format_type: String,
    pub geocoding: Geocoding,
    pub features: Vec<Feature>,
}

impl GeocodeJsonResponse {
    pub fn new(q: String, features: Vec<Feature>) -> Self {
        GeocodeJsonResponse {
            format_type: "FeatureCollection".to_string(),
            geocoding: Geocoding {
                version: "0.1.0".to_string(),
                query: Some(q),
            },
            features,
        }
    }
}

impl FromWithLang<Vec<places::Place>> for GeocodeJsonResponse {
    fn from_with_lang(places: Vec<places::Place>, lang: Option<&str>) -> Self {
        GeocodeJsonResponse::new(
            "".to_string(),
            places
                .into_iter()
                .map(|p| Feature::from_with_lang(p, lang))
                .collect(),
        )
    }
}

#[derive(Serialize, Debug, JsonSchema)]
pub struct Geocoding {
    version: String,
    query: Option<String>,
}

#[derive(Serialize, Debug, JsonSchema)]
pub struct Feature {
    pub r#type: String,
    pub geometry: qwant_geojson::Geometry,
    pub properties: Properties,
    // FIXME distance to the lat lon given in query parameters?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<u32>,
}

#[derive(Serialize, Debug, JsonSchema)]
pub struct Properties {
    pub geocoding: GeocodeJsonProperty,
}

/// This structure contains the result of a geocoding query
/// It adheres to the geocodejson spec
#[derive(Serialize, Debug, JsonSchema)]
pub struct GeocodeJsonProperty {
    pub id: String,
    #[serde(rename = "type")]
    pub place_type: Type,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_type: Option<String>,
    pub label: Option<String>,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub housenumber: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    pub postcode: Option<String>,
    pub city: Option<String>,
    pub citycode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    pub administrative_regions: Vec<AssociatedAdmin>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub poi_types: Vec<places::poi::PoiType>,
    // For retrocompatibility, we can't have just a map of key values,
    // that would be too simple. We need a vector of objects
    // { key: "<key>", value: "<value>" }
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub properties: Vec<KeyValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Box<GeocodeJsonProperty>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub timezone: Option<String>,
    // For retrocompatibility, we can't have just a map of key values,
    // that would be too simple. We need a vector of objects
    // { name: "<name>", value: "<value>" }
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub codes: Vec<NameValue>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bbox: Option<Rect>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub country_codes: Vec<String>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct AssociatedAdmin {
    pub id: String,
    pub insee: String,
    pub level: u32,
    pub label: String,
    pub name: String,
    pub zip_codes: Vec<String>,
    pub coord: places::coord::Coord,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bbox: Option<Rect>,
    #[serde(default)]
    #[schemars(with = "ZoneTypeDef")]
    pub zone_type: Option<cosmogony::ZoneType>,
    #[serde(default)]
    pub parent_id: Option<String>, // id of the Admin's parent (from the cosmogony's hierarchy)
    // For retrocompatibility, we can't have just a map of key values,
    // that would be too simple. We need a vector of objects
    // { name: "<name>", value: "<value>" }
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub codes: Vec<NameValue>,
}

impl FromWithLang<&places::admin::Admin> for AssociatedAdmin {
    fn from_with_lang(admin: &places::admin::Admin, lang: Option<&str>) -> Self {
        let (name, label) = if let Some(code) = lang {
            (
                admin.names.get(code).unwrap_or(&admin.name),
                admin.labels.get(code).unwrap_or(&admin.label),
            )
        } else {
            (admin.name.as_ref(), admin.label.as_ref())
        };
        let codes = admin.codes.iter().fold(Vec::new(), |mut v, (key, value)| {
            v.push(NameValue {
                name: key.to_string(),
                value: value.to_string(),
            });
            v
        });

        AssociatedAdmin {
            bbox: admin.bbox,
            codes,
            coord: admin.coord,
            id: admin.id.clone(),
            insee: admin.insee.clone(),
            label: label.to_string(),
            level: admin.level,
            name: name.to_string(),
            parent_id: admin.parent_id.clone(),
            zip_codes: admin.zip_codes.clone(),
            zone_type: admin.zone_type,
        }
    }
}

pub trait FromWithLang<T> {
    fn from_with_lang(_: T, lang: Option<&str>) -> Self;
}

impl FromWithLang<places::Place> for Feature {
    fn from_with_lang(place: places::Place, lang: Option<&str>) -> Feature {
        let geom = qwant_geojson::Geometry::from(&place);
        let distance = place.distance();
        let geocoding = GeocodeJsonProperty::from_with_lang(place, lang);
        Feature {
            r#type: "Feature".to_string(),
            geometry: geom,
            properties: Properties { geocoding },
            distance,
        }
    }
}

impl FromWithLang<places::admin::Admin> for GeocodeJsonProperty {
    fn from_with_lang(admin: places::admin::Admin, lang: Option<&str>) -> GeocodeJsonProperty {
        let (name, label) = if let Some(lang) = lang {
            (
                admin.names.get(lang).unwrap_or(&admin.name),
                admin.labels.get(lang).unwrap_or(&admin.label),
            )
        } else {
            (admin.name.as_ref(), admin.label.as_ref())
        };

        let zone_type = admin
            .zone_type
            .map(|x| x.as_str().to_case(Case::Snake))
            .unwrap_or_else(|| "administrative_region".to_owned());
        let name = Some(name.to_owned());
        let insee = Some(admin.insee);
        let level = Some(admin.level); //might be used for type_ and become useless
        let postcode = if admin.zip_codes.is_empty() {
            None
        } else {
            Some(admin.zip_codes.join(";"))
        };
        let label = Some(label.to_owned());
        let codes = admin.codes.iter().fold(Vec::new(), |mut v, (key, value)| {
            v.push(NameValue {
                name: key.to_string(),
                value: value.to_string(),
            });
            v
        });

        let associated_admins = admin
            .administrative_regions
            .iter()
            .map(|a| AssociatedAdmin::from_with_lang(a, lang))
            .collect();

        GeocodeJsonProperty {
            address: None,
            administrative_regions: associated_admins,
            bbox: admin.bbox,
            city: None,
            citycode: insee,
            codes,
            country_codes: admin.country_codes,
            housenumber: None,
            id: admin.id,
            label,
            level,
            name,
            place_type: Type::Zone,
            poi_types: vec![],
            postcode,
            properties: vec![],
            street: None,
            timezone: None,
            zone_type: if zone_type.is_empty() {
                None
            } else {
                Some(zone_type)
            },
        }
    }
}

fn get_city_name(admins: &[Arc<places::admin::Admin>]) -> Option<String> {
    admins
        .iter()
        .find(|a| a.is_city())
        .map(|admin| admin.name.clone())
}

fn get_citycode(admins: &[Arc<places::admin::Admin>]) -> Option<String> {
    admins
        .iter()
        .find(|a| a.is_city())
        .map(|admin| admin.insee.clone())
}

impl FromWithLang<places::street::Street> for GeocodeJsonProperty {
    fn from_with_lang(street: places::street::Street, lang: Option<&str>) -> GeocodeJsonProperty {
        let name = Some(street.name);
        let label = Some(street.label);
        let admins = street.administrative_regions;
        let city = get_city_name(&admins);
        let postcode = if street.zip_codes.is_empty() {
            None
        } else {
            Some(street.zip_codes.join(";"))
        };
        let citycode = get_citycode(&admins);

        let associated_admins = admins
            .iter()
            .map(|a| AssociatedAdmin::from_with_lang(a, lang))
            .collect();

        GeocodeJsonProperty {
            address: None,
            administrative_regions: associated_admins,
            bbox: None,
            city,
            citycode,
            codes: vec![],
            country_codes: street.country_codes,
            housenumber: None,
            id: street.id,
            label,
            level: None,
            name: name.clone(),
            place_type: Type::Street,
            poi_types: vec![],
            postcode,
            properties: vec![],
            street: name,
            timezone: None,
            zone_type: None,
        }
    }
}

impl FromWithLang<places::addr::Addr> for GeocodeJsonProperty {
    fn from_with_lang(addr: places::addr::Addr, lang: Option<&str>) -> GeocodeJsonProperty {
        let label = Some(addr.label);
        let housenumber = Some(addr.house_number.to_string());
        let street_name = Some(addr.street.name.to_string());
        let name = Some(addr.name.to_string());
        let admins = addr.street.administrative_regions;
        let city = get_city_name(&admins);
        let postcode = if addr.zip_codes.is_empty() {
            None
        } else {
            Some(addr.zip_codes.join(";"))
        };
        let citycode = get_citycode(&admins);

        let associated_admins = admins
            .iter()
            .map(|a| AssociatedAdmin::from_with_lang(a, lang))
            .collect();

        GeocodeJsonProperty {
            address: None,
            administrative_regions: associated_admins,
            bbox: None,
            city,
            citycode,
            codes: vec![],
            country_codes: addr.country_codes,
            housenumber,
            id: addr.id,
            label,
            level: None,
            name,
            place_type: Type::House,
            poi_types: vec![],
            postcode,
            properties: vec![],
            street: street_name,
            timezone: None,
            zone_type: None,
        }
    }
}

impl FromWithLang<places::poi::Poi> for GeocodeJsonProperty {
    fn from_with_lang(poi: places::poi::Poi, lang: Option<&str>) -> GeocodeJsonProperty {
        let (name, label) = if let Some(code) = lang {
            (
                poi.names.get(code).unwrap_or(&poi.name),
                poi.labels.get(code).unwrap_or(&poi.label),
            )
        } else {
            (poi.name.as_ref(), poi.label.as_ref())
        };
        let name = Some(name.to_owned());
        let label = Some(label.to_owned());
        let admins = poi.administrative_regions;
        let city = get_city_name(&admins);
        let postcode = if poi.zip_codes.is_empty() {
            None
        } else {
            Some(poi.zip_codes.join(";"))
        };
        let citycode = get_citycode(&admins);

        let associated_admins = admins
            .iter()
            .map(|a| AssociatedAdmin::from_with_lang(a, lang))
            .collect();

        let properties = poi
            .properties
            .iter()
            .fold(Vec::new(), |mut v, (key, value)| {
                v.push(KeyValue {
                    key: key.to_string(),
                    value: value.to_string(),
                });
                v
            });

        GeocodeJsonProperty {
            address: match poi.address {
                Some(places::Address::Addr(addr)) => {
                    Some(Box::new(GeocodeJsonProperty::from_with_lang(addr, lang)))
                }
                Some(places::Address::Street(street)) => {
                    Some(Box::new(GeocodeJsonProperty::from_with_lang(street, lang)))
                }
                _ => None,
            },
            administrative_regions: associated_admins,
            bbox: None,
            city,
            citycode,
            codes: vec![],
            country_codes: poi.country_codes,
            housenumber: None,
            id: poi.id,
            label,
            level: None,
            name,
            place_type: Type::Poi,
            poi_types: vec![poi.poi_type],
            postcode,
            properties,
            street: None,
            timezone: None,
            zone_type: None,
        }
    }
}

impl FromWithLang<places::Place> for GeocodeJsonProperty {
    fn from_with_lang(place: places::Place, lang: Option<&str>) -> Self {
        match place {
            places::Place::Admin(admin) => GeocodeJsonProperty::from_with_lang(admin, lang),
            places::Place::Street(street) => GeocodeJsonProperty::from_with_lang(street, lang),
            places::Place::Addr(addr) => GeocodeJsonProperty::from_with_lang(addr, lang),
            places::Place::Poi(poi) => GeocodeJsonProperty::from_with_lang(poi, lang),
        }
    }
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct NameValue {
    pub name: String,
    pub value: String,
}
