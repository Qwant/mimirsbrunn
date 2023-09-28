use geo_types::Point;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors};

use qwant_geojson::Geometry;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema, Validate, Default)]
pub struct Coord {
    #[validate(range(min = - 90, max = 90))]
    lon: f64,
    #[validate(range(min = - 180, max = 180))]
    lat: f64,
}

impl Coord {
    /// Creates a new coordinate without
    pub fn new<T: Into<f64>>(lat: T, lon: T) -> Self {
        Coord {
            lon: lon.into(),
            lat: lat.into(),
        }
    }

    /// Tries to create a new coordinate and validate the latitude and longitude attributes
    pub fn try_new<T: Into<f64>>(lat: T, lon: T) -> Result<Self, ValidationErrors> {
        let coord = Coord {
            lon: lon.into(),
            lat: lat.into(),
        };

        coord.validate()?;
        Ok(coord)
    }

    pub fn lon(&self) -> f64 {
        self.lon
    }

    pub fn lat(&self) -> f64 {
        self.lat
    }
}

impl From<geo_types::Coord<f64>> for Coord {
    fn from(value: geo_types::Coord<f64>) -> Self {
        Self {
            lon: value.x,
            lat: value.y,
        }
    }
}

impl From<Coord> for geo_types::Coord<f64> {
    fn from(value: Coord) -> geo_types::Coord<f64> {
        let coord = (value.lon, value.lat);
        geo_types::Coord::from(coord)
    }
}

impl From<Coord> for Geometry {
    fn from(coord: Coord) -> Geometry {
        Geometry::Point(vec![coord.lon(), coord.lat()])
    }
}

impl From<Coord> for Point {
    fn from(coord: Coord) -> Point {
        Point::new(coord.lon, coord.lat)
    }
}
