use crate::tagger::{Tagger, TaggerAutocomplete};
use crate::ASSETS_PATH;
use bk_tree::BKTree;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use trie_rs::{Trie, TrieBuilder};

pub static CATEGORY_AUTOCOMPLETE_TAGGER: Lazy<CategoryAutocompleteTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("categories.json");
    let categories = fs::read(path).expect("category data");
    let categories: Vec<Category> =
        serde_json::from_slice(&categories).expect("json category data");
    let mut keywords = TrieBuilder::new();
    let mut category_map = HashMap::new();
    for category in categories.into_iter() {
        for keyword in category.matches {
            keywords.push(keyword.to_ascii_lowercase());
            category_map.insert(keyword, category.name.to_owned());
        }
    }

    CategoryAutocompleteTagger {
        keywords: keywords.build(),
        category_map,
    }
});

/// A tagger to categorize input query
/// ```rust
/// use tagger::{CATEGORY_AUTOCOMPLETE_TAGGER, TaggerAutocomplete};
///
/// assert_eq!(
///     CATEGORY_AUTOCOMPLETE_TAGGER.tag("mair"),
///     Some("administrative".to_string())
/// );
/// ```
pub struct CategoryAutocompleteTagger {
    // All possible keyword searchable with levenshtein distance
    keywords: Trie<u8>,
    // Reverse hashmap to find a category from a matched keyword
    category_map: HashMap<String, String>,
}

impl TaggerAutocomplete for CategoryAutocompleteTagger {
    type Output = Option<String>;

    fn tag(&self, input: &str) -> Self::Output {
        self.keywords
            .predictive_search(input)
            .iter()
            .map(|u8s| std::str::from_utf8(u8s).unwrap())
            .next()
            .and_then(|keyword| self.category_map.get(keyword).cloned())
    }
}

pub static CATEGORY_TAGGER: Lazy<CategoryTagger> = Lazy::new(|| {
    let assets =
        ASSETS_PATH.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let path = assets.join("categories.json");
    let categories = fs::read(path).expect("category data");
    let categories: Vec<Category> =
        serde_json::from_slice(&categories).expect("json category data");
    let mut keywords = BKTree::default();
    let mut category_map = HashMap::new();
    for category in categories.into_iter() {
        for keyword in category.matches {
            keywords.add(keyword.to_ascii_lowercase());
            category_map.insert(keyword, category.name.to_owned());
        }
    }

    CategoryTagger {
        keywords,
        category_map,
    }
});

#[derive(Deserialize, Debug)]
struct Category {
    name: String,
    matches: Vec<String>,
}

/// A tagger to categorize input query
/// ```rust
/// use tagger::{CATEGORY_TAGGER, Tagger};
///
/// assert_eq!(
///     CATEGORY_TAGGER.tag("mairie", Some(1)),
///     Some("administrative".to_string())
/// );
/// ```
pub struct CategoryTagger {
    // All possible keyword searchable with levenshtein distance
    keywords: BKTree<String>,
    // Reverse hashmap to find a category from a matched keyword
    category_map: HashMap<String, String>,
}

impl Tagger for CategoryTagger {
    type Output = Option<String>;

    fn tag(&self, input: &str, tolerance: Option<u32>) -> Self::Output {
        self.keywords
            .find(input, tolerance.unwrap_or(0))
            .next()
            .and_then(|(_, keyword)| self.category_map.get(keyword).cloned())
    }
}

#[cfg(test)]
mod test {
    use crate::tagger::category::CATEGORY_TAGGER;
    use crate::tagger::Tagger;

    #[test]
    fn category_tagger_works() {
        assert_eq!(
            CATEGORY_TAGGER.tag("restau chinois", Some(1)),
            Some("food_chinese".to_string())
        );
    }
}
