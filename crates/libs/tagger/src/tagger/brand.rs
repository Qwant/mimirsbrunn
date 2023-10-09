use crate::tagger::{Tagger, TaggerAutocomplete};
use crate::ASSETS_PATH;
use bk_tree::BKTree;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use trie_rs::{Trie, TrieBuilder};

pub static BRAND_AUTOCOMPLETE_TAGGER: Lazy<BrandAutocompleteTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("brand.json");
    let brands = fs::read(path).expect("brands data");
    let brands: Vec<Brand> = serde_json::from_slice(&brands).expect("json brand data");
    let mut tree = TrieBuilder::new();

    brands
        .into_iter()
        .for_each(|brand| tree.push(brand.name.to_ascii_lowercase()));

    BrandAutocompleteTagger {
        inner: tree.build(),
    }
});

/// A tagger to find brands in prefix query
/// ```rust
/// use tagger::{BRAND_AUTOCOMPLETE_TAGGER, TaggerAutocomplete};
///
///  assert_eq!(BRAND_AUTOCOMPLETE_TAGGER.tag("app"), true);
/// ```
pub struct BrandAutocompleteTagger {
    inner: Trie<u8>,
}

impl TaggerAutocomplete for BrandAutocompleteTagger {
    type Output = bool;
    fn tag(&self, input: &str) -> bool {
        !self.inner.predictive_search(input).is_empty()
    }
}

pub static BRAND_TAGGER: Lazy<BrandTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("brand.json");
    let brands = fs::read(path).expect("brands data");
    let brands: Vec<Brand> = serde_json::from_slice(&brands).expect("json brand data");
    let mut tree = BKTree::default();

    brands
        .into_iter()
        .for_each(|brand| tree.add(brand.name.to_ascii_lowercase()));

    BrandTagger { inner: tree }
});

#[derive(Deserialize, Debug)]
struct Brand {
    name: String,
}

/// A tagger to find brands in input query
/// ```rust
/// use tagger::{BRAND_TAGGER, Tagger};
///
///  assert_eq!(BRAND_TAGGER.tag("apple", Some(0)), true);
/// ```
pub struct BrandTagger {
    inner: BKTree<String>,
}

impl Tagger for BrandTagger {
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
    use crate::tagger::brand::BRAND_TAGGER;
    use crate::tagger::Tagger;

    #[test]
    fn brand_tagger_works() {
        assert!(BRAND_TAGGER.tag("gamm vert", Some(1)));
        assert!(!BRAND_TAGGER.tag("apple nike", Some(1)));
        assert!(BRAND_TAGGER.tag("apple", Some(0)));
    }
}
