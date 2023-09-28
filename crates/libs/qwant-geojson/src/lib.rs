use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod conversion;
pub mod errors;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(untagged)]
pub enum GeoJson {
    Geometry(Geometry),
    Feature(Feature),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(untagged)]
pub enum FeatureId {
    String(String),
    Number(serde_json::Number),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(tag = "type")]
pub enum Feature {
    GeometryCollection {
        geometries: Vec<Geometry>,
    },
    Feature {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<FeatureId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bbox: Option<Vec<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        geometry: Option<Geometry>,
        properties: Value,
    },
    FeatureCollection {
        #[serde(skip_serializing_if = "Option::is_none")]
        bbox: Option<Vec<f64>>,
        features: Vec<Feature>,
    },
}

pub type PointType = Vec<f64>;
pub type MultiPointType = Vec<PointType>;
pub type LineStringType = Vec<PointType>;
pub type MultiLineStringType = Vec<LineStringType>;
pub type PolygonType = Vec<Vec<PointType>>;
pub type MultiPolygonType = Vec<PolygonType>;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(tag = "type", content = "coordinates")]
pub enum Geometry {
    Point(PointType),

    /// MultiPoint
    ///
    /// [GeoJSON Format Specification § 3.1.3](https://tools.ietf.org/html/rfc7946#section-3.1.3)
    MultiPoint(MultiPointType),

    /// LineString
    ///
    /// [GeoJSON Format Specification § 3.1.4](https://tools.ietf.org/html/rfc7946#section-3.1.4)
    LineString(LineStringType),

    /// MultiLineString
    ///
    /// [GeoJSON Format Specification § 3.1.5](https://tools.ietf.org/html/rfc7946#section-3.1.5)
    MultiLineString(MultiLineStringType),

    /// Polygon
    ///
    /// [GeoJSON Format Specification § 3.1.6](https://tools.ietf.org/html/rfc7946#section-3.1.6)
    Polygon(PolygonType),

    /// MultiPolygon
    ///
    /// [GeoJSON Format Specification § 3.1.7](https://tools.ietf.org/html/rfc7946#section-3.1.7)
    MultiPolygon(MultiPolygonType),

    /// GeometryCollection
    ///
    /// [GeoJSON Format Specification § 3.1.8](https://tools.ietf.org/html/rfc7946#section-3.1.8)
    GeometryCollection {
        geometries: Vec<Geometry>,
    },
}

impl Geometry {
    fn type_name(&self) -> &'static str {
        match self {
            Geometry::Point(_) => "Point",
            Geometry::MultiPoint(_) => "MultiPoint",
            Geometry::LineString(_) => "LineString",
            Geometry::MultiLineString(_) => "MultiLineString",
            Geometry::Polygon(_) => "Polygon",
            Geometry::MultiPolygon(_) => "MultiPolygon",
            Geometry::GeometryCollection { .. } => "GeometryCollection",
        }
    }
}
