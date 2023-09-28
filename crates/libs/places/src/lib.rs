use qwant_geojson::Geometry;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod addr;
pub mod admin;
pub mod code;
pub mod context;
pub mod coord;
pub mod i18n_properties;
pub mod poi;
pub mod rect;
pub mod street;
pub mod utils;

use addr::Addr;
use admin::Admin;
use poi::Poi;
use street::Street;

/// Generic document.
pub trait Document: DeserializeOwned + Serialize {
    // TODO: Do we need to use an owned string here?
    /// Unique identifier for the document.
    fn id(&self) -> String;
}

/// A type of document with a fixed type.
///
/// A collection of this kind of document has a consistent schema and can hence
/// be used to generate a container.
pub trait ContainerDocument: Document {
    fn static_doc_type() -> &'static str;
}

/// Object stored in Elasticsearch
#[allow(clippy::large_enum_variant)]
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Place {
    Admin(Admin),
    Street(Street),
    Addr(Addr),
    Poi(Poi),
}

/// There are two kinds of addresses:
/// Note that the enum is 'untagged' with regards to serde because
/// each of `Addr` and `Street` already has a 'type' field.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", untagged)]
pub enum Address {
    Street(Street),
    Addr(Addr),
}

impl Place {
    pub fn is_admin(&self) -> bool {
        matches!(self, Place::Admin(_))
    }

    pub fn is_street(&self) -> bool {
        matches!(self, Place::Street(_))
    }

    pub fn is_addr(&self) -> bool {
        matches!(self, Place::Addr(_))
    }

    pub fn is_poi(&self) -> bool {
        matches!(self, Place::Poi(_))
    }

    pub fn poi(&self) -> Option<&Poi> {
        match *self {
            Place::Poi(ref poi) => Some(poi),
            _ => None,
        }
    }

    pub fn id(&self) -> &str {
        match *self {
            Place::Admin(ref o) => &o.id,
            Place::Street(ref o) => &o.id,
            Place::Addr(ref o) => &o.id,
            Place::Poi(ref o) => &o.id,
        }
    }

    pub fn label(&self) -> &str {
        match *self {
            Place::Admin(ref o) => o.label(),
            Place::Street(ref o) => o.label(),
            Place::Addr(ref o) => o.label(),
            Place::Poi(ref o) => o.label(),
        }
    }

    pub fn admins(&self) -> Vec<Arc<Admin>> {
        match *self {
            Place::Admin(ref o) => o.admins(),
            Place::Street(ref o) => o.admins(),
            Place::Addr(ref o) => o.admins(),
            Place::Poi(ref o) => o.admins(),
        }
    }

    pub fn address(&self) -> Option<Address> {
        match *self {
            Place::Admin(_) => None,
            Place::Street(ref o) => Some(Address::Street(o.clone())),
            Place::Addr(ref o) => Some(Address::Addr(o.clone())),
            Place::Poi(_) => None,
        }
    }

    pub fn distance(&self) -> Option<u32> {
        match *self {
            Place::Admin(ref o) => o.distance,
            Place::Street(ref o) => o.distance,
            Place::Addr(ref o) => o.distance,
            Place::Poi(ref o) => o.distance,
        }
    }

    pub fn set_distance(&mut self, d: u32) {
        match self {
            Place::Admin(ref mut o) => o.distance = Some(d),
            Place::Street(ref mut o) => o.distance = Some(d),
            Place::Addr(ref mut o) => o.distance = Some(d),
            Place::Poi(ref mut o) => o.distance = Some(d),
        }
    }

    pub fn coord(&self) -> &coord::Coord {
        match self {
            Place::Admin(ref o) => &o.coord,
            Place::Street(ref o) => &o.coord,
            Place::Addr(ref o) => &o.coord,
            Place::Poi(ref o) => &o.coord,
        }
    }

    pub fn set_context(&mut self, context: context::Context) {
        match self {
            Place::Admin(ref mut o) => o.context = Some(context),
            Place::Street(ref mut o) => o.context = Some(context),
            Place::Addr(ref mut o) => o.context = Some(context),
            Place::Poi(ref mut o) => o.context = Some(context),
        }
    }

    /* We can afford to clone the context because we're in debug mode
     * and performance are less critical */
    pub fn context(&self) -> Option<context::Context> {
        match self {
            Place::Admin(ref o) => o.context.clone(),
            Place::Street(ref o) => o.context.clone(),
            Place::Addr(ref o) => o.context.clone(),
            Place::Poi(ref o) => o.context.clone(),
        }
    }
}

// This is a bit of a kludge to a get a string version for the doc_type.
#[derive(PartialEq, Copy, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlaceDocType {
    Admin,
    Street,
    Addr,
    Poi,
}

impl PlaceDocType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlaceDocType::Admin => "admin",
            PlaceDocType::Street => "street",
            PlaceDocType::Addr => "addr",
            PlaceDocType::Poi => "poi",
        }
    }
}

pub trait Members {
    fn label(&self) -> &str;
    fn admins(&self) -> Vec<Arc<Admin>>;
}

#[derive(
    Serialize, Deserialize, JsonSchema, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Default,
)]
pub struct Property {
    pub key: String,
    pub value: String,
}

impl From<&Place> for Geometry {
    fn from(place: &Place) -> Self {
        match place {
            Place::Admin(admin) => Geometry::from(admin),
            Place::Street(street) => Geometry::from(street),
            Place::Addr(addr) => Geometry::from(addr),
            Place::Poi(poi) => Geometry::from(poi),
        }
    }
}
