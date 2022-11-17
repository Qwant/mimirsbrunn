use async_trait::async_trait;
use cucumber::{then, when};
use geo::algorithm::haversine_distance::HaversineDistance;
use itertools::{EitherOrBoth::*, Itertools};
use mimir::{
    adapters::secondary::elasticsearch::remote::connection_test_pool,
    domain::{model::configuration, ports::secondary::remote::Remote},
};
use std::cmp::Ordering;

use crate::{
    error::Error,
    state::{GlobalState, State, Step, StepStatus},
    steps::TEST_INDEX_ROOT,
};
use mimir::{
    adapters::{
        primary::{
            bragi::api::DEFAULT_LIMIT_RESULT_ES,
            common::{
                coord::Coord,
                dsl::{build_query, QueryType},
                filters::Filters,
                settings::QuerySettings,
            },
        },
        secondary::elasticsearch::ElasticsearchStorageConfig,
    },
    domain::{model::query::Query, ports::primary::search_documents::SearchDocuments},
};
use places::{addr::Addr, admin::Admin, poi::Poi};

// Search place

#[when(regex = r#"the user searches (.+) datatype for "(.*)" at (.+), (.+)$"#)]
async fn search(state: &mut GlobalState, places: String, query: String, lat: f32, lon: f32) {
    perform_search(state, places, query, Coord::new(lat, lon).into(), None).await;
}

#[when(regex = r#"the user searches (.+) datatype for "(.+)" with "(.+)" filters$"#)]
async fn search_with_zone_filters(
    state: &mut GlobalState,
    places: String,
    query: String,
    zone_types: String,
) {
    perform_search(state, places, query, None, Some(zone_types)).await;
}

#[when(regex = r#"the user searches (.+) datatype for "(.*)"$"#)]
async fn search_no_coord(state: &mut GlobalState, places: String, query: String) {
    perform_search(state, places, query, None, None).await;
}

async fn perform_search(
    state: &mut GlobalState,
    places: String,
    query: String,
    coord: Option<Coord>,
    zone_types: Option<String>,
) {
    let places = {
        if places == "all" {
            vec![TEST_INDEX_ROOT.to_string()]
        } else {
            places
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .map(|s| configuration::root_doctype(TEST_INDEX_ROOT, &s))
                .collect()
        }
    };

    let zone_types = zone_types.map(|f| {
        f.split(',')
            .map(str::trim)
            .map(str::to_string)
            .collect::<Vec<String>>()
    });

    let filters = Filters {
        coord,
        zone_types,
        ..Default::default()
    };

    state
        .execute(Search {
            places,
            query,
            filters,
            results: Vec::new(),
        })
        .await
        .expect("failed to search");
}

/// Perform a search in current Elasticsearch DB.
#[derive(Debug)]
pub struct Search {
    places: Vec<String>, // What kind of places we're searching
    query: String,       // The search string
    filters: Filters,    // Search filters
    results: Vec<serde_json::Value>,
}

#[async_trait(?Send)]
impl Step for Search {
    async fn execute(&mut self, _state: &State) -> Result<StepStatus, Error> {
        let client = connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Could not establish connection to Elasticsearch");

        // Build ES query
        let dsl = build_query(
            TEST_INDEX_ROOT,
            &self.query,
            &self.filters,
            "fr",
            &QuerySettings::default(),
            QueryType::PREFIX,
            None,
        );

        // Fetch documents
        self.results = {
            client
                .search_documents(
                    self.places.clone(),
                    Query::QueryDSL(dsl),
                    DEFAULT_LIMIT_RESULT_ES,
                    None,
                )
                .await
                .unwrap()
        };

        Ok(StepStatus::Done)
    }
}

// Find document

#[then(regex = r#"he finds "(.+)" as the first result$"#)]
async fn find_id(state: &mut GlobalState, id: String) {
    state
        .execute(HasDocument { id, max_rank: 1 })
        .await
        .expect("failed to find document");
}

/// Check if given document is in the output.
///
/// It assumes that a Search has already been performed before.
pub struct HasDocument {
    id: String,
    max_rank: usize,
}

#[async_trait(?Send)]
impl Step for HasDocument {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let (search, _) = state
            .steps_for::<Search>()
            .next_back()
            .expect("the user must perform a search before checking results");

        let (rank, _) = (search.results.iter().enumerate())
            .find(|(_, doc)| {
                println!("{} vs {}", self.id, doc["id"].as_str().unwrap());
                self.id == doc["id"].as_str().unwrap()
            })
            .expect("document was not found in search results");

        assert!(rank < self.max_rank);
        Ok(StepStatus::Done)
    }
}

// Find Admin

#[then(regex = r#"he finds admin "(.*)", a "(.*)", in the first (.*) results$"#)]
async fn find_admin(state: &mut GlobalState, name: String, zone_type: String, limit: usize) {
    state
        .execute(HasAdmin {
            name: Some(name),
            zone_type: Some(zone_type),
            limit,
        })
        .await
        .expect("failed to find document");
}

/// Check if given admin is in the output.
///
/// It assumes that a Search has already been performed before.
pub struct HasAdmin {
    name: Option<String>,
    zone_type: Option<String>,
    limit: usize,
}

#[async_trait(?Send)]
impl Step for HasAdmin {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let (search, _) = state
            .steps_for::<Search>()
            .next_back()
            .expect("the user must perform a search before checking results");

        let rank = search
            .results
            .clone()
            .into_iter()
            .enumerate()
            .find_map(|(num, doc)| {
                let admin: Admin = serde_json::from_value(doc).expect("admin");
                if let Some(name) = &self.name {
                    if !equal_ignore_case_utf8(name, admin.name.as_str()) {
                        return None;
                    }
                }
                if let Some(zone_type) = &self.zone_type {
                    if !equal_ignore_case_utf8(
                        zone_type,
                        admin.zone_type.expect("zone type").as_str(),
                    ) {
                        // is it justified to expect a zone type?
                        return None;
                    }
                }
                Some(num)
            })
            .expect("document was not found in search results");

        assert!(rank < self.limit);
        Ok(StepStatus::Done)
    }
}

// Find address

#[then(
    regex = r#"he finds address "(.*)", "(.*)", "(.*)", and "(.*)" in the first "(.*)" results$"#
)]
async fn find_address(
    state: &mut GlobalState,
    house: String,
    street: String,
    city: String,
    postcode: String,
    limit: usize,
) {
    state
        .execute(HasAddress {
            house: none_if_empty(house),
            street: none_if_empty(street),
            city: none_if_empty(city),
            postcode: none_if_empty(postcode),
            limit,
        })
        .await
        .expect("failed to find document");
}

/// Check if given address is in the output.
///
/// It assumes that a Search has already been performed before.
pub struct HasAddress {
    house: Option<String>,
    street: Option<String>,
    city: Option<String>,
    postcode: Option<String>,
    limit: usize,
}

#[async_trait(?Send)]
impl Step for HasAddress {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let (search, _) = state
            .steps_for::<Search>()
            .next_back()
            .expect("the user must perform a search before checking results");

        let rank = search
            .results
            .clone()
            .into_iter()
            .enumerate()
            .find_map(|(num, doc)| {
                let addr: Addr = serde_json::from_value(doc).expect("address");
                if let Some(house) = &self.house {
                    if !equal_ignore_case_utf8(house, addr.house_number.as_str()) {
                        return None;
                    }
                }
                if let Some(street) = &self.street {
                    if !equal_ignore_case_utf8(street, addr.street.name.as_str()) {
                        return None;
                    }
                }
                if let Some(postcode) = &self.postcode {
                    if !equal_ignore_case_utf8(postcode, addr.zip_codes[0].as_str()) {
                        return None;
                    }
                }
                if let Some(city) = &self.city {
                    if !equal_ignore_case_utf8(city, addr.city().expect("city").as_str()) {
                        return None;
                    }
                }
                Some(num)
            })
            .expect("document was not found in search results");

        assert!(rank < self.limit);
        Ok(StepStatus::Done)
    }
}

// Find POI

#[then(
    regex = r#"he finds poi "(.*)", a "(.*)" located near ([0-9\.]+), ([0-9\.]+) in the first (\d+) results$"#
)]
async fn find_poi(
    state: &mut GlobalState,
    label: String,
    poi_type: String,
    lat: f64,
    lon: f64,
    limit: usize,
) {
    state
        .execute(HasPoi {
            label: none_if_empty(label),
            poi_type: none_if_empty(poi_type),
            coord: Some(geo_types::Point::new(lon, lat)),
            limit,
        })
        .await
        .expect("failed to find document");
}

/// Check if given poi is in the output.
///
/// It assumes that a Search has already been performed before.
pub struct HasPoi {
    label: Option<String>,
    poi_type: Option<String>,
    coord: Option<geo_types::Point<f64>>,
    limit: usize,
}

#[async_trait(?Send)]
impl Step for HasPoi {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let (search, _) = state
            .steps_for::<Search>()
            .next_back()
            .expect("the user must perform a search before checking results");

        let rank = search
            .results
            .clone()
            .into_iter()
            .enumerate()
            .find_map(|(num, doc)| {
                let poi: Poi = serde_json::from_value(doc).expect("poi");
                if let Some(label) = &self.label {
                    if !equal_ignore_case_utf8(label, poi.label.as_str()) {
                        return None;
                    }
                }
                if let Some(poi_type) = &self.poi_type {
                    if !equal_ignore_case_utf8(poi_type, poi.poi_type.name.as_str()) {
                        return None;
                    }
                }
                if let Some(coord) = &self.coord {
                    let distance = coord.haversine_distance(&poi.coord.into());
                    // if we are at more than 100m
                    if distance > 100.0 {
                        return None;
                    }
                }
                Some(num)
            })
            .expect("document was not found in search results");

        assert!(rank < self.limit);
        Ok(StepStatus::Done)
    }
}

// Utils

fn none_if_empty(val: String) -> Option<String> {
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

// Based on https://stackoverflow.com/questions/63871601/what-is-an-efficient-way-to-compare-strings-while-ignoring-case
// Its a case insensitive comparison, returning true if equal, false otherwise
fn equal_ignore_case_utf8(a: &str, b: &str) -> bool {
    let abs = a
        .chars()
        .flat_map(char::to_lowercase)
        .zip_longest(b.chars().flat_map(char::to_lowercase));

    for ab in abs {
        match ab {
            Left(_) => return false,
            Right(_) => return false,
            Both(a, b) => {
                if a.cmp(&b) != Ordering::Equal {
                    return false;
                }
            }
        }
    }
    true
}
