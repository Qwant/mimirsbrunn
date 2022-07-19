// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
//
// LICENCE: This program is free software; you can redistribute it
// and/or modify it under the terms of the GNU Affero General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public
// License along with this program. If not, see
// <http://www.gnu.org/licenses/>.
//
// Stay tuned using
// twitter @navitia
// IRC #navitia on freenode
// https://groups.google.com/d/forum/navitia
// www.navitia.io

use geo::algorithm::{
    bounding_rect::BoundingRect, contains::Contains, euclidean_distance::EuclideanDistance,
    intersects::Intersects,
};
use geo_types::{MultiPolygon, Point};
use mimir::adapters::secondary::elasticsearch::ElasticsearchStorage;
use places::admin::Admin;
use rstar::{Envelope, PointDistance, RTree, RTreeObject, SelectionFunction, AABB};
use std::{
    collections::{HashMap, HashSet},
    iter::{Extend, FromIterator},
    sync::Arc,
};
use tracing::{info, warn};

use crate::{admin, admin::fetch_admins, settings::admin_settings::AdminSettings};

// This is a structure which is used in the RTree to customize the list of objects returned
// when searching at a given location. This version just focuses on the envelope of the object,
// for performance reason. The call envelope.contains() is much cheaper than boundary.contains().
// On the other hand, there is a chance that the point is in the envelope but not in the boundary.
struct PointInEnvelopeSelectionFunction<T>
where
    T: RTreeObject,
{
    point: <T::Envelope as Envelope>::Point,
}

impl<T> SelectionFunction<T> for PointInEnvelopeSelectionFunction<T>
where
    T: RTreeObject,
{
    fn should_unpack_parent(&self, envelope: &T::Envelope) -> bool {
        envelope.contains_point(&self.point)
    }

    fn should_unpack_leaf(&self, leaf: &T) -> bool {
        leaf.envelope().contains_point(&self.point)
    }
}

// This is the object stored in the RTree.
// It splits the admin, taking the boundary in one field, and the rest as an Arc.
// We store the envelope so we don't have to recompute it every time we query this bounded id
pub struct SplitAdmin {
    pub envelope: AABB<[f64; 2]>,
    pub boundary: MultiPolygon<f64>,
    pub admin: Arc<Admin>,
}

// This trait is needed so that SplitAdmin can be inserted in the RTree
impl RTreeObject for SplitAdmin {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

impl PointDistance for SplitAdmin {
    // This function computes the square of the distance from an Admin to a point.
    // We compute the distance from the boundary to a point.
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let p = Point::new(point[0], point[1]);
        let d = self.boundary.euclidean_distance(&p);
        d * d
    }

    // contains_point is provided, but we override the default implementation using
    // the geo algorithms for performance, as suggested in rstar documentation.
    fn contains_point(&self, point: &[f64; 2]) -> bool {
        let p = Point::new(point[0], point[1]);
        self.boundary.contains(&p)
    }
}

// In the AdminGeoFinder, we need search for admins in two ways:
// Geographically, so we'll use an RTree.
// Using an id
pub struct AdminGeoFinder {
    rtree: RTree<SplitAdmin>,
    admin_by_id: HashMap<String, Arc<Admin>>,
}

impl AdminGeoFinder {
    pub async fn build(
        admin_settings: &AdminSettings,
        client: &ElasticsearchStorage,
    ) -> Result<AdminGeoFinder, admin::Error> {
        let admins = fetch_admins(admin_settings, client).await?;
        let geofinder: AdminGeoFinder = admins.into_iter().collect();
        Ok(geofinder)
    }

    pub fn insert(&mut self, admin: Admin) {
        let mut admin = admin;
        let boundary = std::mem::replace(&mut admin.boundary, None);
        match boundary {
            Some(boundary) => match boundary.bounding_rect() {
                Some(bb) => {
                    let admin = Arc::new(admin);
                    let split = SplitAdmin {
                        envelope: AABB::from_corners(
                            [bb.min().x, bb.min().y],
                            [bb.max().x, bb.max().y],
                        ),
                        boundary,
                        admin: admin.clone(),
                    };
                    self.admin_by_id.insert(admin.id.clone(), admin);
                    self.rtree.insert(split);
                }
                None => warn!("Admin '{}' has a boundary but no bounding box", admin.id),
            },
            None => info!(
                "Admin '{}' has no boundary (=> not inserted in the AdminGeoFinder)",
                admin.id
            ),
        }
    }

    // Get all admins overlapping the given coordinates verifying the condition.
    //
    // Each element of the returned array contains a leaf of the admin hierarchy
    // that overlap input coordinates together with its parents.
    pub fn get_admins_if(
        &self,
        coord: &geo_types::Coordinate<f64>,
        condition: impl Fn(&Admin) -> bool,
    ) -> Vec<Vec<Arc<Admin>>> {
        // Get a list of overlapping admins whose zone_type is less than granularity
        let mut candidates = self
            .rtree
            .locate_with_selection_function(PointInEnvelopeSelectionFunction {
                point: [coord.x, coord.y],
            })
            .filter(|cand| condition(&cand.admin))
            .collect::<Vec<_>>();

        // Keep track of the admins that have already been visited as a parent. In such
        // a case we don't need to check if it needs to be returned as it will be part
        // of the input as parent of another admin.
        let mut visited_ids = HashSet::new();

        candidates.sort_by_key(|cand| cand.admin.zone_type);
        candidates
            .into_iter()
            .filter_map(move |cand| {
                let admin = cand.admin.clone();
                let bound = &cand.boundary;

                if !visited_ids.contains(admin.id.as_str())
                    && bound.intersects(&geo_types::Point(*coord))
                {
                    let mut res = vec![admin];

                    while let Some(parent) = res
                        .last()
                        .unwrap()
                        .parent_id
                        .as_ref()
                        .and_then(|parent_id| self.admin_by_id.get(parent_id.as_str()))
                    {
                        visited_ids.insert(parent.id.as_str());
                        res.push(parent.clone());
                    }

                    Some(res)
                } else {
                    None
                }
            })
            .collect()
    }

    // Get all Admins overlapping the given coordinates.
    // Finding if a point is within a complex boundary, such as a multipolygon, is
    // expensive, compared to finding if it is in the envelope of the boundary.
    // So this function works in two stages:
    // (1) First it finds the list of admins that _may_ contain the coordinates.
    //     These admins's bbox contains the coordinates, but it does not mean the
    //     admin actually contains the coordinates. We sort these admins by size.
    //     We call them 'candidates'.
    // (2) We then iterate through these candidates and see if we already have the
    //     hierarchy which may have been previlously computed by eg. cosmogony.
    pub fn get(&self, coord: &geo_types::Coordinate<f64>) -> Vec<Arc<Admin>> {
        let selection_function = PointInEnvelopeSelectionFunction {
            point: [coord.x, coord.y],
        };
        // Get a list of overlapping admins...
        let mut candidates = self
            .rtree
            .locate_with_selection_function(selection_function)
            .collect::<Vec<_>>();

        // We sort them so we can start with the smallest zone_type.
        candidates.sort_by_key(|adm| adm.admin.zone_type);

        let mut tested_hierarchy = HashSet::new();
        let mut added_zone_types = HashSet::new();
        let mut res = vec![];

        for candidate in candidates {
            let boundary = &candidate.boundary;
            let admin = &candidate.admin;
            if tested_hierarchy.contains(&admin.id) {
                res.push(admin.clone());
            } else if admin
                .zone_type
                .as_ref()
                .map_or(false, |zt| added_zone_types.contains(zt))
            {
                // we don't want it, we already have this kind of ZoneType
            } else if boundary.contains(&geo_types::Point(*coord)) {
                // we found a valid admin, we save it's hierarchy not to have to test their boundaries
                if let Some(zt) = admin.zone_type {
                    added_zone_types.insert(zt);
                }
                let mut admin_parent_id = admin.parent_id.clone();
                while let Some(id) = admin_parent_id {
                    let admin_parent = self.admin_by_id.get(&id);
                    if let Some(zt) = admin_parent.as_ref().and_then(|a| a.zone_type) {
                        added_zone_types.insert(zt);
                    }
                    if !tested_hierarchy.insert(id) {
                        break; // stop the exploration of the hierarchy since we have already added this one
                    }
                    admin_parent_id = admin_parent.and_then(|a| a.parent_id.clone());
                }

                res.push(admin.clone());
            }
        }
        res
    }

    /// Return an iterator over admins.
    /// Since we can't modify admins once they are stored in the RTree,
    /// and since this method requires Admins to have their boundary, we create
    /// new admins by cloning the ones in the RTree, and adding their boundary.
    /// Needless to say, this is probably an expensive method...
    pub fn admins(&self) -> impl Iterator<Item = Admin> + '_ {
        self.rtree.iter().map(|split| {
            let mut admin = Admin::clone(&split.admin);
            admin.boundary = Some(split.boundary.clone());
            admin
        })
    }
}

impl Default for AdminGeoFinder {
    fn default() -> Self {
        AdminGeoFinder {
            rtree: RTree::new(),
            admin_by_id: HashMap::new(),
        }
    }
}

impl Extend<Admin> for AdminGeoFinder {
    fn extend<T: IntoIterator<Item = Admin>>(&mut self, admins: T) {
        for mut admin in admins {
            admin.administrative_regions = Vec::new();
            self.insert(admin);
        }
    }
}

impl FromIterator<Admin> for AdminGeoFinder {
    fn from_iter<I: IntoIterator<Item = Admin>>(admins: I) -> Self {
        let mut geofinder = AdminGeoFinder::default();
        geofinder.extend(admins);
        geofinder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmogony::ZoneType;
    use geo::prelude::BoundingRect;
    use places::coord::Coord;

    fn p(x: f64, y: f64) -> geo_types::Point<f64> {
        geo_types::Point(geo_types::Coordinate { x, y })
    }

    fn make_admin(offset: f64, zt: Option<ZoneType>) -> Admin {
        make_complex_admin(&format!("admin:offset:{}", offset,), offset, zt, 1., None)
    }

    fn make_complex_admin(
        id: &str,
        offset: f64,
        zone_type: Option<ZoneType>,
        zone_size: f64,
        parent_offset: Option<&str>,
    ) -> Admin {
        // the boundary is a big octogon
        // the zone_size param is used to control the area of the zone
        let shape = geo_types::Polygon::new(
            geo_types::LineString(vec![
                (3. * zone_size + offset, 0. * zone_size + offset).into(), //     ^
                (6. * zone_size + offset, 0. * zone_size + offset).into(), //     |   x   x
                (9. * zone_size + offset, 3. * zone_size + offset).into(), //     |
                (9. * zone_size + offset, 6. * zone_size + offset).into(), //     x           x
                (6. * zone_size + offset, 9. * zone_size + offset).into(), //     |
                (3. * zone_size + offset, 9. * zone_size + offset).into(), //     x           x
                (0. * zone_size + offset, 6. * zone_size + offset).into(), //     |
                (0. * zone_size + offset, 3. * zone_size + offset).into(), //     +---x---x------->
                (3. * zone_size + offset, 0. * zone_size + offset).into(), //
            ]),
            vec![],
        );
        let boundary = geo_types::MultiPolygon(vec![shape]);

        let coord = Coord::new(4.0 + offset, 4.0 + offset);
        Admin {
            id: id.into(),
            level: 8,
            name: "city".to_string(),
            label: format!("city {}", offset),
            zip_codes: vec!["421337".to_string()],
            weight: 0f64,
            coord,
            approx_coord: Some(coord.into()),
            bbox: boundary.bounding_rect(),
            boundary: Some(boundary),
            insee: "outlook".to_string(),
            zone_type,
            parent_id: parent_offset.map(|id| id.into()),
            ..Default::default()
        }
    }

    #[test]
    fn test_two_fake_admins() {
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_admin(40., Some(ZoneType::City)));
        finder.insert(make_admin(43., Some(ZoneType::State)));

        // outside
        for coord in [p(48., 41.), p(411., 41.), p(51., 54.), p(53., 53.)].iter() {
            assert!(finder.get(&coord.0).is_empty());
        }

        // inside one
        let admins = finder.get(&p(44., 44.).0);
        assert_eq!(admins.len(), 1);
        assert_eq!(admins[0].id, "admin:offset:40");
        let admins = finder.get(&p(48., 48.).0);
        assert_eq!(admins.len(), 1);
        assert_eq!(admins[0].id, "admin:offset:43");

        // inside two
        let mut admins = finder.get(&p(46., 46.).0);
        admins.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(admins.len(), 2);
        assert_eq!(admins[0].id, "admin:offset:40");
        assert_eq!(admins[1].id, "admin:offset:43");
    }

    #[test]
    fn test_two_admin_same_zone_type() {
        // a point can be associated to only 1 admin type
        // so a point is in 2 city, it is associated to only one
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_admin(40., Some(ZoneType::City)));
        finder.insert(make_admin(43., Some(ZoneType::City)));
        let admins = finder.get(&p(46., 46.).0);
        assert_eq!(admins.len(), 1);
    }

    #[test]
    fn test_two_no_zone_type() {
        // a point can be associated to only 1 admin type
        // but a point can be associated to multiple admin without zone_type
        // (for retrocompatibility of the data imported without cosmogony)
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_admin(40., None));
        finder.insert(make_admin(43., None));
        let admins = finder.get(&p(46., 46.).0);
        assert_eq!(admins.len(), 2);
    }

    #[test]
    fn test_hierarchy() {
        // In this test we use 3 admin regions that include each other.
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_complex_admin(
            "bob_city",
            40.,
            Some(ZoneType::City),
            1.,
            Some("bob_state"),
        ));
        finder.insert(make_complex_admin(
            "bob_state",
            40.,
            Some(ZoneType::StateDistrict),
            2.,
            Some("bob_country"),
        ));
        finder.insert(make_complex_admin(
            "bob_country",
            40.,
            Some(ZoneType::Country),
            3.,
            None,
        ));

        let admins = finder.get(&p(46., 46.).0);
        assert_eq!(admins.len(), 3);
        assert_eq!(admins[0].id, "bob_city");
        assert_eq!(admins[1].id, "bob_state");
        assert_eq!(admins[2].id, "bob_country");
    }

    #[test]
    fn test_hierarchy_orphan() {
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_complex_admin(
            "bob_city",
            40.,
            Some(ZoneType::City),
            1.,
            Some("bob_state"),
        ));
        finder.insert(make_complex_admin(
            "bob_state",
            40.,
            Some(ZoneType::StateDistrict),
            2.,
            Some("bob_country"),
        ));
        finder.insert(make_complex_admin(
            "bob_country",
            40.,
            Some(ZoneType::Country),
            3.,
            None,
        ));

        // another_state also contains the point, but the geofinder look for only 1 admin by type (it needs only 1 state)
        // since bob_city has been tester first, it's hierarchy has been added automatically
        // so [46., 46.] will not be associated to another_state
        finder.insert(make_complex_admin(
            "another_state",
            40.,
            Some(ZoneType::StateDistrict),
            2.,
            Some("bob_country"),
        ));

        let admins = finder.get(&p(46., 46.).0);
        assert_eq!(admins.len(), 3);
        assert_eq!(admins[0].id, "bob_city");
        assert_eq!(admins[1].id, "bob_state");
        assert_eq!(admins[2].id, "bob_country");
    }

    #[test]
    fn test_hierarchy_and_not_typed_zone() {
        let mut finder = AdminGeoFinder::default();
        finder.insert(make_complex_admin(
            "bob_city",
            40.,
            Some(ZoneType::City),
            1.,
            Some("bob_state"),
        ));
        finder.insert(make_complex_admin(
            "bob_state",
            40.,
            Some(ZoneType::StateDistrict),
            2.,
            Some("bob_country"),
        ));
        finder.insert(make_complex_admin(
            "bob_country",
            40.,
            Some(ZoneType::Country),
            3.,
            None,
        ));

        // not_typed zone is outside the hierarchy, but since it contains the point and it has no type it is added
        finder.insert(make_complex_admin("no_typed_zone", 40., None, 2., None));

        let admins = finder.get(&p(46., 46.).0);
        assert_eq!(admins.len(), 4);
        assert_eq!(admins[0].id, "no_typed_zone");
        assert_eq!(admins[1].id, "bob_city");
        assert_eq!(admins[2].id, "bob_state");
        assert_eq!(admins[3].id, "bob_country");
    }
}
