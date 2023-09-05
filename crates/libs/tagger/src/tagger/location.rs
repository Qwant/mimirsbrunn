use crate::tagger::Tagger;
use crate::tokens::normalize_diacritics;
use crate::ASSETS_PATH;
use bk_tree::BKTree;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

pub static CITY_TAGGER: Lazy<CityTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("ville_france.json");
    let locations = fs::read(path).expect("cities data");
    let locations: Cities = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.cities.into_iter().for_each(|city| {
        let city = normalize_diacritics(&city);

        tree.add(city)
    });

    CityTagger { inner: tree }
});

pub static CAPITAL_CITY_TAGGER: Lazy<CapitalCityTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("capital_fr.json");
    let locations = fs::read(path).expect("capital cities data");
    let locations: CapitalCities =
        serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.capital_cities.into_iter().for_each(|city| {
        let city = normalize_diacritics(&city);

        tree.add(city)
    });

    CapitalCityTagger { inner: tree }
});

pub static COUNTRIES_TAGGER: Lazy<CountriesTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("countries.json");
    let locations = fs::read(path).expect("countries data");
    let locations: Countries = serde_json::from_slice(&locations).expect("valid json locations");

    let mut tree = BKTree::default();

    locations.countries.into_iter().for_each(|city| {
        let city = normalize_diacritics(&city);

        tree.add(city)
    });

    CountriesTagger { inner: tree }
});

#[derive(Debug, Deserialize)]
struct Cities {
    cities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CapitalCities {
    capital_cities: Vec<String>,
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

pub struct CapitalCityTagger {
    inner: BKTree<String>,
}

pub struct CountriesTagger {
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

impl Tagger for CapitalCityTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

impl Tagger for CountriesTagger {
    type Output = bool;
    fn tag(&self, input: &str, tolerance: Option<u32>) -> bool {
        self.inner
            .find(input, tolerance.unwrap_or(0))
            .next()
            .is_some()
    }
}

#[cfg(test)]
mod test {
    use crate::tagger::location::{CAPITAL_CITY_TAGGER, CITY_TAGGER, COUNTRIES_TAGGER};
    use crate::tagger::Tagger;

    /// These test are meant to be run locally since needs linking to libpostal
    #[test]
    fn city_tagger_works() {
        assert!(CITY_TAGGER.tag("brest", Some(0)));
        assert!(CITY_TAGGER.tag("poiseul la grange", Some(0)));
        assert!(!CITY_TAGGER.tag("asteroid city", Some(0)));
        assert!(CITY_TAGGER.tag("franconville la garenne", Some(0)));
    }

    #[test]
    fn capital_city_tagger_works() {
        assert!(CAPITAL_CITY_TAGGER.tag("paris", Some(0)));
        assert!(CAPITAL_CITY_TAGGER.tag("londres", Some(0)));
        assert!(!CAPITAL_CITY_TAGGER.tag("lille", Some(0)));
    }

    #[test]
    fn countries_tagger_works() {
        assert!(COUNTRIES_TAGGER.tag("espagne", Some(0)));
        assert!(COUNTRIES_TAGGER.tag("germany", Some(0)));
        assert!(!COUNTRIES_TAGGER.tag("norvège", Some(0)));
    }
}
