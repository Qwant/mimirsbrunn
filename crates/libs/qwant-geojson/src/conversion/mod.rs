// Copyright 2015 The GeoRust Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//  http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::errors::Result;
use crate::{Feature, GeoJson};
use geo_types::{self, CoordFloat, GeometryCollection};
use std::convert::TryInto;

pub(crate) mod from_geo_types;
pub(crate) mod to_geo_types;
// Process top-level `GeoJSON` items, returning a geo_types::GeometryCollection or an Error
fn process_geojson<T>(gj: &GeoJson) -> Result<GeometryCollection<T>>
where
    T: CoordFloat,
{
    match gj {
        GeoJson::Geometry(geometry) => Ok(GeometryCollection(vec![geometry.clone().try_into()?])),
        GeoJson::Feature(feature) => {
            match feature {
                Feature::Feature { geometry, .. } => {
                    if let Some(geometry) = &geometry {
                        Ok(GeometryCollection(vec![geometry.clone().try_into()?]))
                    } else {
                        Ok(GeometryCollection(vec![]))
                    }
                }
                Feature::FeatureCollection { features, .. } => Ok(GeometryCollection(
                    features
                        .iter()
                        // Only pass on non-empty geometries
                        .filter_map(|feature| match feature {
                            Feature::Feature { geometry, .. } => geometry.as_ref(),
                            _ => None,
                        })
                        .map(|geometry| geometry.clone().try_into())
                        .collect::<Result<_>>()?,
                )),
                Feature::GeometryCollection { geometries } => {
                    let geometries = geometries
                        .iter()
                        .map(|g| g.try_into())
                        .filter_map(Result::ok)
                        .collect();

                    Ok(GeometryCollection(geometries))
                }
            }
        }
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "geo-types")))]
pub fn quick_collection<T>(gj: &GeoJson) -> Result<geo_types::GeometryCollection<T>>
where
    T: CoordFloat,
{
    process_geojson(gj)
}
