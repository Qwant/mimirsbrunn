use geojson::Geometry;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::warn;
use transit_model::objects::Rgb;
use typed_index_collection::Idx;

use super::context::Context;
use super::coord::Coord;
use super::Members;
use super::{ContainerDocument, Document};
use crate::admin::Admin;
use crate::utils::normalize_id;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct CommercialMode {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalMode {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Network {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Default)]
pub struct Line {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commercial_mode: Option<CommercialMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
    pub physical_modes: Vec<PhysicalMode>,
    #[serde(skip_serializing)]
    pub sort_order: Option<u32>, // we do not serialise this field, it is only used to sort the Lines
}

pub trait FromTransitModel<T> {
    fn from_transit_model(idx: Idx<T>, navitia: &transit_model::Model) -> Self;
}

impl FromTransitModel<transit_model::objects::Line> for Line {
    fn from_transit_model(
        l_idx: Idx<transit_model::objects::Line>,
        navitia: &transit_model::Model,
    ) -> Self {
        let line = &navitia.lines[l_idx];
        Self {
            id: normalize_id("line", &line.id),
            name: line.name.clone(),
            code: line.code.clone(),
            color: line.color.clone(),
            sort_order: line.sort_order,
            text_color: line.text_color.clone(),
            commercial_mode: navitia
                .commercial_modes
                .get(&line.commercial_mode_id)
                .map(|c| CommercialMode {
                    id: normalize_id("commercial_mode", &c.id),
                    name: c.name.clone(),
                }),
            network: navitia.networks.get(&line.network_id).map(|n| Network {
                id: normalize_id("network", &n.id),
                name: n.name.clone(),
            }),
            physical_modes: navitia
                .get_corresponding_from_idx(l_idx)
                .into_iter()
                .map(|p_idx| {
                    let physical_mode = &navitia.physical_modes[p_idx];
                    PhysicalMode {
                        id: normalize_id("physical_mode", &physical_mode.id),
                        name: physical_mode.name.clone(),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct FeedPublisher {
    pub id: String,
    pub license: String,
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Comment {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(tag = "type", rename = "stop")]
pub struct Stop {
    pub id: String,
    pub label: String,
    pub name: String,
    /// coord used for some geograhic queries in ES, less precise but  faster than `coord`
    /// https://www.elastic.co/guide/en/elasticsearch/reference/2.4/geo-shape.html
    #[serde(skip_deserializing)]
    pub approx_coord: Option<Geometry>,
    pub coord: Coord,
    pub administrative_regions: Vec<Arc<Admin>>,
    pub weight: f64,
    pub zip_codes: Vec<String>,
    #[serde(default)]
    pub commercial_modes: Vec<CommercialMode>,
    #[serde(default)]
    pub physical_modes: Vec<PhysicalMode>,
    #[serde(default)]
    pub coverages: Vec<String>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub codes: BTreeMap<String, String>,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    #[serde(default)]
    pub feed_publishers: Vec<FeedPublisher>,
    /// Distance to the coord in query.
    /// Not serialized as is because it is returned in the `Feature` object
    #[serde(default, skip)]
    pub distance: Option<u32>,
    #[serde(default)]
    pub lines: Vec<Line>,
    #[serde(default)]
    pub country_codes: Vec<String>,

    pub context: Option<Context>,
}

impl Members for Stop {
    fn label(&self) -> &str {
        &self.label
    }
    fn admins(&self) -> Vec<Arc<Admin>> {
        self.administrative_regions.clone()
    }
}

impl Document for Stop {
    fn id(&self) -> String {
        self.id.clone()
    }
}

impl ContainerDocument for Stop {
    fn static_doc_type() -> &'static str {
        "stop"
    }
}

fn get_lines(
    idx: Idx<transit_model::objects::StopArea>,
    navitia: &transit_model::Model,
) -> Vec<Line> {
    // use FromTransitModel;
    let mut lines: Vec<_> = navitia
        .get_corresponding_from_idx(idx)
        .into_iter()
        .map(|l_idx| Line::from_transit_model(l_idx, navitia))
        .collect();

    // we want the lines to be sorted in a way where
    // line-3 is before line-11, so be use a human_sort
    lines.sort_by(|lhs, rhs| {
        match (&lhs.sort_order, &rhs.sort_order) {
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(s), Some(o)) => s.cmp(o),
            (None, None) => Ordering::Equal,
        }
        .then_with(|| match (&lhs.code, &rhs.code) {
            (Some(l), Some(r)) => human_sort::compare(l, r),
            _ => Ordering::Equal,
        })
        .then_with(|| human_sort::compare(&lhs.name, &rhs.name))
    });
    lines
}

pub fn to_mimir(
    idx: Idx<transit_model::objects::StopArea>,
    stop_area: &transit_model::objects::StopArea,
    navitia: &transit_model::Model,
) -> Stop {
    let commercial_modes = navitia
        .get_corresponding_from_idx(idx)
        .into_iter()
        .map(|cm_idx| CommercialMode {
            id: normalize_id("commercial_mode", &navitia.commercial_modes[cm_idx].id),
            name: navitia.commercial_modes[cm_idx].name.clone(),
        })
        .collect();
    let physical_modes = navitia
        .get_corresponding_from_idx(idx)
        .into_iter()
        .map(|pm_idx| PhysicalMode {
            id: normalize_id("physical_mode", &navitia.physical_modes[pm_idx].id),
            name: navitia.physical_modes[pm_idx].name.clone(),
        })
        .collect();
    let comments = stop_area
        .comment_links
        .iter()
        .filter_map(|comment_id| {
            let res = navitia.comments.get(comment_id);
            if res.is_none() {
                warn!("Could not retrieve comments for id {}", comment_id);
            }
            res
        })
        .map(|comment| Comment {
            name: comment.name.clone(),
        })
        .collect();
    let feed_publishers = navitia
        .get_corresponding_from_idx(idx)
        .into_iter()
        .map(|contrib_idx| FeedPublisher {
            id: navitia.contributors[contrib_idx].id.clone(),
            name: navitia.contributors[contrib_idx].name.clone(),
            license: navitia.contributors[contrib_idx]
                .license
                .clone()
                .unwrap_or_else(|| "".into()),
            url: navitia.contributors[contrib_idx]
                .website
                .clone()
                .unwrap_or_else(|| "".into()),
        })
        .collect();

    let coord = Coord::new(stop_area.coord.lon, stop_area.coord.lat)
        .expect("invalid coordinate for stop area");

    let lines = get_lines(idx, navitia);

    Stop {
        id: normalize_id("stop_area", &stop_area.id),
        label: stop_area.name.clone(),
        name: stop_area.name.clone(),
        coord,
        approx_coord: Some(coord.into()),
        commercial_modes,
        physical_modes,
        lines,
        comments,
        timezone: stop_area
            .timezone
            .map(chrono_tz::Tz::name)
            .map(str::to_owned)
            .unwrap_or_default(),
        codes: stop_area
            .codes
            .iter()
            .map(|(t, v)| (t.clone(), v.clone()))
            .collect(),
        properties: stop_area.object_properties.clone(),
        feed_publishers,
        ..Default::default()
    }
}

impl From<&Stop> for geojson::Geometry {
    fn from(stop: &Stop) -> Self {
        geojson::Geometry::from(stop.coord)
    }
}
