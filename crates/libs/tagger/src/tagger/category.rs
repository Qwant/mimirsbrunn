use crate::tagger::Tagger;
use crate::ASSETS_PATH;
use bk_tree::BKTree;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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
