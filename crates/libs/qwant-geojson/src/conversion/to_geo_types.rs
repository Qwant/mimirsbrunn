use crate::conversion::quick_collection;
use crate::errors::{Error, Result};
use crate::{Feature, GeoJson, Geometry, LineStringType, PointType, PolygonType};
use geo_types::{self, CoordFloat, Point};
use std::convert::{TryFrom, TryInto};

impl<T> TryFrom<&Geometry> for Point<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::Point(point_type) => Ok(create_geo_point(point_type)),
            other => Err(mismatch_geom_err("Point", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for Point<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::MultiPoint<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::MultiPoint(multi_point_type) => Ok(geo_types::MultiPoint(
                multi_point_type
                    .iter()
                    .map(|point_type| create_geo_point(point_type))
                    .collect(),
            )),
            other => Err(mismatch_geom_err("MultiPoint", other)),
        }
    }
}
impl<T: CoordFloat> TryFrom<Geometry> for geo_types::MultiPoint<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::LineString<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::LineString(multi_point_type) => Ok(create_geo_line_string(multi_point_type)),
            other => Err(mismatch_geom_err("LineString", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::LineString<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::MultiLineString<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::MultiLineString(multi_line_string_type) => {
                Ok(create_geo_multi_line_string(multi_line_string_type))
            }
            other => Err(mismatch_geom_err("MultiLineString", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::MultiLineString<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::Polygon<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::Polygon(polygon_type) => Ok(create_geo_polygon(polygon_type)),
            other => Err(mismatch_geom_err("Polygon", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::Polygon<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::MultiPolygon<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<geo_types::MultiPolygon<T>> {
        match value {
            Geometry::MultiPolygon(multi_polygon_type) => {
                Ok(create_geo_multi_polygon(multi_polygon_type))
            }
            other => Err(mismatch_geom_err("MultiPolygon", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::MultiPolygon<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::GeometryCollection<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::GeometryCollection { geometries } => {
                let geojson_geometries = geometries
                    .iter()
                    .map(|geometry| geometry.try_into().unwrap())
                    .collect();

                Ok(geo_types::GeometryCollection(geojson_geometries))
            }
            other => Err(mismatch_geom_err("GeometryCollection", other)),
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::GeometryCollection<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<&Geometry> for geo_types::Geometry<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(value: &Geometry) -> Result<Self> {
        match value {
            Geometry::Point(ref point_type) => {
                Ok(geo_types::Geometry::Point(create_geo_point(point_type)))
            }
            Geometry::MultiPoint(ref multi_point_type) => {
                Ok(geo_types::Geometry::MultiPoint(geo_types::MultiPoint(
                    multi_point_type
                        .iter()
                        .map(|point_type| create_geo_point(point_type))
                        .collect(),
                )))
            }
            Geometry::LineString(ref line_string_type) => Ok(geo_types::Geometry::LineString(
                create_geo_line_string(line_string_type),
            )),
            Geometry::MultiLineString(ref multi_line_string_type) => {
                Ok(geo_types::Geometry::MultiLineString(
                    create_geo_multi_line_string(multi_line_string_type),
                ))
            }
            Geometry::Polygon(ref polygon_type) => Ok(geo_types::Geometry::Polygon(
                create_geo_polygon(polygon_type),
            )),
            Geometry::MultiPolygon(ref multi_polygon_type) => Ok(
                geo_types::Geometry::MultiPolygon(create_geo_multi_polygon(multi_polygon_type)),
            ),
            Geometry::GeometryCollection { ref geometries } => {
                let gc = geo_types::Geometry::GeometryCollection(geo_types::GeometryCollection(
                    geometries
                        .iter()
                        .cloned()
                        .map(|geom| geom.try_into())
                        .collect::<Result<Vec<geo_types::Geometry<T>>>>()?,
                ));
                Ok(gc)
            }
        }
    }
}

impl<T: CoordFloat> TryFrom<Geometry> for geo_types::Geometry<T> {
    type Error = Error;

    fn try_from(value: Geometry) -> Result<Self> {
        (&value).try_into()
    }
}

impl<T> TryFrom<Feature> for geo_types::Geometry<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(val: Feature) -> Result<geo_types::Geometry<T>> {
        Ok(geo_types::Geometry::GeometryCollection(quick_collection(
            &GeoJson::Feature(val),
        )?))
    }
}

impl<T> TryFrom<GeoJson> for geo_types::Geometry<T>
where
    T: CoordFloat,
{
    type Error = Error;

    fn try_from(val: GeoJson) -> Result<geo_types::Geometry<T>> {
        match val {
            GeoJson::Geometry(geom) => geom.try_into(),
            GeoJson::Feature(feat) => feat.try_into(),
        }
    }
}

fn create_geo_coordinate<T>(point_type: &PointType) -> geo_types::Coord<T>
where
    T: CoordFloat,
{
    geo_types::Coord {
        x: T::from(point_type[0]).unwrap(),
        y: T::from(point_type[1]).unwrap(),
    }
}

fn create_geo_point<T>(point_type: &PointType) -> geo_types::Point<T>
where
    T: CoordFloat,
{
    geo_types::Point::new(
        T::from(point_type[0]).unwrap(),
        T::from(point_type[1]).unwrap(),
    )
}

fn create_geo_line_string<T>(line_type: &LineStringType) -> geo_types::LineString<T>
where
    T: CoordFloat,
{
    geo_types::LineString(
        line_type
            .iter()
            .map(|point_type| create_geo_coordinate(point_type))
            .collect(),
    )
}

fn create_geo_multi_line_string<T>(
    multi_line_type: &[LineStringType],
) -> geo_types::MultiLineString<T>
where
    T: CoordFloat,
{
    geo_types::MultiLineString(
        multi_line_type
            .iter()
            .map(|point_type| create_geo_line_string(point_type))
            .collect(),
    )
}

fn create_geo_polygon<T>(polygon_type: &PolygonType) -> geo_types::Polygon<T>
where
    T: CoordFloat,
{
    let exterior = polygon_type
        .get(0)
        .map(|e| create_geo_line_string(e))
        .unwrap_or_else(|| create_geo_line_string(&vec![]));

    let interiors = if polygon_type.len() < 2 {
        vec![]
    } else {
        polygon_type[1..]
            .iter()
            .map(|line_string_type| create_geo_line_string(line_string_type))
            .collect()
    };

    geo_types::Polygon::new(exterior, interiors)
}

fn create_geo_multi_polygon<T>(multi_polygon_type: &[PolygonType]) -> geo_types::MultiPolygon<T>
where
    T: CoordFloat,
{
    geo_types::MultiPolygon(
        multi_polygon_type
            .iter()
            .map(|polygon_type| create_geo_polygon(polygon_type))
            .collect(),
    )
}

fn mismatch_geom_err(expected_type: &'static str, found: &Geometry) -> Error {
    Error::InvalidGeometryConversion {
        expected_type,
        found_type: found.type_name(),
    }
}
