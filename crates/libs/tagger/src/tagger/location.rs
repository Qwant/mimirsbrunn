use crate::tagger::Tagger;
use crate::tokens::normalize_diacritics;
use crate::ASSETS_PATH;
use bk_tree::BKTree;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

pub static COUNTRY_TAGGER: Lazy<CountryTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("countries.json");
    let locations = fs::read(path).expect("countries data");
    let locations: Countries = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.countries.into_iter().for_each(|country| {
        let country = normalize_diacritics(&country);

        tree.add(country)
    });

    CountryTagger { inner: tree }
});

pub static STATE_TAGGER: Lazy<StateTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("states.json");
    let locations = fs::read(path).expect("state data");
    let locations: States = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.states.into_iter().for_each(|state| {
        let state = normalize_diacritics(&state);

        tree.add(state)
    });

    StateTagger { inner: tree }
});

pub static DISTRICT_TAGGER: Lazy<DistrictTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("districts.json");
    let locations = fs::read(path).expect("district data");
    let locations: Districts = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.districts.into_iter().for_each(|district| {
        let district = normalize_diacritics(&district);

        tree.add(district)
    });

    DistrictTagger { inner: tree }
});

pub static CITY_TAGGER: Lazy<CityTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("cities.json");
    let locations = fs::read(path).expect("cities data");
    let locations: Cities = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.cities.into_iter().for_each(|city| {
        let city = normalize_diacritics(&city);

        tree.add(city)
    });

    CityTagger { inner: tree }
});

pub static CITY_DISTRICT_TAGGER: Lazy<CityDistrictTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("cities_districts.json");
    let locations = fs::read(path).expect("city district data");
    let locations: CitiesDistricts =
        serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations
        .cities_districts
        .into_iter()
        .for_each(|city_district| {
            let city_district = normalize_diacritics(&city_district);

            tree.add(city_district)
        });

    CityDistrictTagger { inner: tree }
});

pub static SUBURBS_TAGGER: Lazy<SuburbTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("suburbs.json");
    let locations = fs::read(path).expect("suburb data");
    let locations: Suburbs = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.suburbs.into_iter().for_each(|suburbs| {
        let suburbs = normalize_diacritics(&suburbs);

        tree.add(suburbs)
    });

    SuburbTagger { inner: tree }
});

#[derive(Debug, Deserialize)]
struct Cities {
    cities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct States {
    states: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Districts {
    districts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CitiesDistricts {
    cities_districts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Suburbs {
    suburbs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Countries {
    countries: Vec<String>,
}

/// A tagger to find cities in input query
/// ```rust, ignore
/// use tagger::{CITY_TAGGER, Tagger};
///
///  assert_eq!(CITY_TAGGER.tag("brest", 0), true);
/// ```
pub struct CityTagger {
    inner: BKTree<String>,
}

pub struct StateTagger {
    inner: BKTree<String>,
}

pub struct DistrictTagger {
    inner: BKTree<String>,
}

pub struct CityDistrictTagger {
    inner: BKTree<String>,
}

pub struct SuburbTagger {
    inner: BKTree<String>,
}

pub struct CountryTagger {
    inner: BKTree<String>,
}

impl Tagger for CityTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for StateTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for DistrictTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for CityDistrictTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for SuburbTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for CountryTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

// #[cfg(test)]
// mod test {
// use crate::tagger::location::{CITY_TAGGER, COUNTRY_TAGGER, DISTRICT_TAGGER};
// use crate::tagger::Tagger;

// These test are meant to be run locally since needs linking to libpostal
// #[test]
// fn city_tagger_works() {
//     assert!(CITY_TAGGER.tag("brest", Some(0)));
//     assert!(CITY_TAGGER.tag("poiseul la grange", Some(0)));
//     assert!(!CITY_TAGGER.tag("asteroid city", Some(0)));
//     assert!(CITY_TAGGER.tag("franconville la garenne", Some(0)));
// }

// #[test]
// fn capital_city_tagger_works() {
//     assert!(CITY_TAGGER.tag("paris", Some(0)));
//     assert!(CITY_TAGGER.tag("londres", Some(0)));
//     assert!(!CITY_TAGGER.tag("lille", Some(0)));
// }
//
// #[test]
// fn countries_tagger_works() {
//     assert!(COUNTRY_TAGGER.tag("espagne", Some(0)));
//     assert!(COUNTRY_TAGGER.tag("germany", Some(0)));
//     assert!(!COUNTRY_TAGGER.tag("norvège", Some(0)));
// }

// #[test]
// fn states_tagger_works() {
//     assert!(STATE_TAGGER.tag("Pays de la Loire", Some(0)));
//     assert!(STATE_TAGGER.tag("Auvergne-Rhône-Alpes", Some(0)));
//     assert!(STATE_TAGGER.tag("Auvergne Rhone Alpes", Some(0)));
// }

// #[test]
// fn districts_tagger_works() {
//     assert!(DISTRICT_TAGGER.tag("rhone", Some(0)));
//     assert!(DISTRICT_TAGGER.tag("seine saint denis", Some(0)));
// }

// #[test]
// fn cities_districts_tagger_works() {
//     assert!(CITY_DISTRICT_TAGGER.tag("lyon 9e", Some(0)));
//     assert!(CITY_DISTRICT_TAGGER.tag("vaugneray", Some(0)));
// }
//
// #[test]
// fn suburbs_tagger_works() {
//     assert!(SUBURBS_TAGGER.tag("Gros Caillou", Some(0)));
//     assert!(SUBURBS_TAGGER.tag("Quartier latin", Some(0)));
// }
// }
