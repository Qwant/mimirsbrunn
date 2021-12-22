use std::time::Duration;

use super::coord::Coord;
use geojson::Geometry;

// How to restrict the range of the query... Except for the place type (ie what indices we're
// searching, since we use the list of types to create the list of indices to search for just
// before calling search_documents.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub coord: Option<Coord>,
    pub shape: Option<(Geometry, Vec<String>)>, // We use String rather than Type to avoid dependencies toward bragi api.
    pub zone_types: Option<Vec<String>>,
    pub poi_types: Option<Vec<String>>,
    pub limit: i64,
    pub timeout: Option<Duration>,
}
