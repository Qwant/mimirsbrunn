use geo_types::{self, CoordFloat};

use crate::{Feature, Geometry};

use crate::{LineStringType, PointType, PolygonType};
use std::convert::From;

impl<'a, T> From<&'a geo_types::Point<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(point: &geo_types::Point<T>) -> Self {
        let coords = create_point_type(point);

        Geometry::Point(coords)
    }
}

impl<'a, T> From<&'a geo_types::MultiPoint<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(multi_point: &geo_types::MultiPoint<T>) -> Self {
        let coords = multi_point
            .0
            .iter()
            .map(|point| create_point_type(point))
            .collect();

        Geometry::MultiPoint(coords)
    }
}

impl<'a, T> From<&'a geo_types::LineString<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(line_string: &geo_types::LineString<T>) -> Self {
        let coords = create_line_string_type(line_string);

        Geometry::LineString(coords)
    }
}

impl<'a, T> From<&'a geo_types::Line<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(line: &geo_types::Line<T>) -> Self {
        let coords = create_from_line_type(line);

        Geometry::LineString(coords)
    }
}

impl<'a, T> From<&'a geo_types::Triangle<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(triangle: &geo_types::Triangle<T>) -> Self {
        let coords = create_from_triangle_type(triangle);

        Geometry::Polygon(coords)
    }
}

impl<'a, T> From<&'a geo_types::Rect<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(rect: &geo_types::Rect<T>) -> Self {
        let coords = create_from_rect_type(rect);

        Geometry::Polygon(coords)
    }
}

impl<'a, T> From<&'a geo_types::MultiLineString<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(multi_line_string: &geo_types::MultiLineString<T>) -> Self {
        let coords = create_multi_line_string_type(multi_line_string);

        Geometry::MultiLineString(coords)
    }
}

impl<'a, T> From<&'a geo_types::Polygon<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(polygon: &geo_types::Polygon<T>) -> Self {
        let coords = create_polygon_type(polygon);

        Geometry::Polygon(coords)
    }
}

impl<'a, T> From<&'a geo_types::MultiPolygon<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(multi_polygon: &geo_types::MultiPolygon<T>) -> Self {
        let coords = create_multi_polygon_type(multi_polygon);

        Geometry::MultiPolygon(coords)
    }
}

impl<'a, T> From<&'a geo_types::GeometryCollection<T>> for Geometry
where
    T: CoordFloat,
{
    fn from(geometry_collection: &geo_types::GeometryCollection<T>) -> Self {
        let geometries = geometry_collection.0.iter().map(Geometry::from).collect();

        Geometry::GeometryCollection { geometries }
    }
}

impl<'a, T> From<&'a geo_types::GeometryCollection<T>> for Feature
where
    T: CoordFloat,
{
    fn from(geometry_collection: &geo_types::GeometryCollection<T>) -> Self {
        let values: Vec<Feature> = geometry_collection
            .0
            .iter()
            .map(|geometry| Feature::Feature {
                id: None,
                bbox: None,
                geometry: Some(geometry.into()),
                properties: Default::default(),
            })
            .collect();

        Feature::FeatureCollection {
            bbox: None,
            features: values,
        }
    }
}

impl<'a, T> From<&'a geo_types::Geometry<T>> for Geometry
where
    T: CoordFloat,
{
    /// Convert from `geo_types::Geometry` enums
    fn from(geometry: &'a geo_types::Geometry<T>) -> Self {
        match *geometry {
            geo_types::Geometry::Point(ref point) => Geometry::from(point),
            geo_types::Geometry::MultiPoint(ref multi_point) => Geometry::from(multi_point),
            geo_types::Geometry::LineString(ref line_string) => Geometry::from(line_string),
            geo_types::Geometry::Line(ref line) => Geometry::from(line),
            geo_types::Geometry::Triangle(ref triangle) => Geometry::from(triangle),
            geo_types::Geometry::Rect(ref rect) => Geometry::from(rect),
            geo_types::Geometry::GeometryCollection(ref gc) => Geometry::from(gc),
            geo_types::Geometry::MultiLineString(ref multi_line_string) => {
                Geometry::from(multi_line_string)
            }
            geo_types::Geometry::Polygon(ref polygon) => Geometry::from(polygon),
            geo_types::Geometry::MultiPolygon(ref multi_polygon) => Geometry::from(multi_polygon),
        }
    }
}

fn create_point_type<T>(point: &geo_types::Point<T>) -> PointType
where
    T: CoordFloat,
{
    let x: f64 = point.x().to_f64().unwrap();
    let y: f64 = point.y().to_f64().unwrap();

    vec![x, y]
}

fn create_line_string_type<T>(line_string: &geo_types::LineString<T>) -> LineStringType
where
    T: CoordFloat,
{
    line_string
        .points()
        .map(|point| create_point_type(&point))
        .collect()
}

fn create_from_line_type<T>(line_string: &geo_types::Line<T>) -> LineStringType
where
    T: CoordFloat,
{
    vec![
        create_point_type(&line_string.start_point()),
        create_point_type(&line_string.end_point()),
    ]
}

fn create_from_triangle_type<T>(triangle: &geo_types::Triangle<T>) -> PolygonType
where
    T: CoordFloat,
{
    create_polygon_type(&triangle.to_polygon())
}

fn create_from_rect_type<T>(rect: &geo_types::Rect<T>) -> PolygonType
where
    T: CoordFloat,
{
    create_polygon_type(&rect.to_polygon())
}

fn create_multi_line_string_type<T>(
    multi_line_string: &geo_types::MultiLineString<T>,
) -> Vec<LineStringType>
where
    T: CoordFloat,
{
    multi_line_string
        .0
        .iter()
        .map(|line_string| create_line_string_type(line_string))
        .collect()
}

fn create_polygon_type<T>(polygon: &geo_types::Polygon<T>) -> PolygonType
where
    T: CoordFloat,
{
    let mut coords = vec![polygon
        .exterior()
        .points()
        .map(|point| create_point_type(&point))
        .collect()];

    coords.extend(
        polygon
            .interiors()
            .iter()
            .map(|line_string| create_line_string_type(line_string)),
    );

    coords
}

fn create_multi_polygon_type<T>(multi_polygon: &geo_types::MultiPolygon<T>) -> Vec<PolygonType>
where
    T: CoordFloat,
{
    multi_polygon
        .0
        .iter()
        .map(|polygon| create_polygon_type(polygon))
        .collect()
}
