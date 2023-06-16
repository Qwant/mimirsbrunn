use geo_types::{Coord, MultiPolygon, Rect};
use geojson::Geometry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::warn;

pub fn serialize_rect<S>(bbox: &Option<Rect<f64>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match bbox {
        Some(b) => {
            // bbox serialized as an array
            // using GeoJSON bounding box format
            // See RFC 7946: https://tools.ietf.org/html/rfc7946#section-5
            let geojson_bbox: geojson::Bbox = vec![b.min().x, b.min().y, b.max().x, b.max().y];
            geojson_bbox.serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

pub fn deserialize_rect<'de, D>(d: D) -> Result<Option<Rect<f64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<f64>>::deserialize(d).map(|option| {
        option.map(|b| {
            Rect::new(
                Coord { x: b[0], y: b[1] }, // min
                Coord { x: b[2], y: b[3] }, // max
            )
        })
    })
}

pub fn custom_multi_polygon_serialize<S>(
    multi_polygon_option: &Option<MultiPolygon<f64>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use geojson::{GeoJson, Value};

    match *multi_polygon_option {
        Some(ref multi_polygon) => {
            GeoJson::Geometry(Geometry::new(Value::from(multi_polygon))).serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

pub fn custom_multi_polygon_deserialize<'de, D>(d: D) -> Result<Option<MultiPolygon<f64>>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    Option::<geojson::GeoJson>::deserialize(d).map(|option| {
        option.and_then(|geojson| match geojson {
            geojson::GeoJson::Geometry(geojson_geometry) => {
                let res: Result<MultiPolygon<f64>, _> = geojson_geometry.value.try_into();
                match res {
                    Ok(multi_polygon) => Some(multi_polygon),
                    Err(err) => {
                        warn!("Cannot deserialize into MultiPolygon: {}", err);
                        None
                    }
                }
            }
            _ => None,
        })
    })
}

pub fn get_country_code(codes: &BTreeMap<String, String>) -> Option<String> {
    codes.get("ISO3166-1:alpha2").cloned()
}

// This function reformat the id by removing spaces, and prepending a prefix
pub fn normalize_id(prefix: &str, id: &str) -> String {
    match prefix {
        "stop_area" => format!(
            "{}:{}",
            prefix,
            &id.replacen("StopArea:", "", 1).replace(' ', "")
        ),
        _ => format!("{}:{}", prefix, &id.replace(' ', "")),
    }
}
