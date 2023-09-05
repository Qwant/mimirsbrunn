use crate::tokens::Tokenized;
use once_cell::sync::OnceCell;
use std::path::PathBuf;

pub mod errors;
mod tagger;
mod tokens;

#[cfg(feature = "postal")]
pub use crate::tagger::address::{AddressTag, ADDRESS_TAGGER};
pub use crate::tokens::normalize_diacritics;

use crate::tagger::location::COUNTRIES_TAGGER;
pub use crate::tagger::{
    brand::BRAND_TAGGER, category::CATEGORY_TAGGER, location::CAPITAL_CITY_TAGGER,
    location::CITY_TAGGER, Tag, TaggedPart, Tagger,
};

pub use crate::tokens::{Span, Tokenizer};

/// Path to the assets directory, if not provided at runtime will default to ./libs/tagger/assets
pub static ASSETS_PATH: OnceCell<PathBuf> = OnceCell::new();

const MAX_DISTANCE_FOR_LEN: (u32, usize) = (1, 6);

#[derive(Default, Debug)]
pub struct TaggerQueryBuilder {
    brands: bool,
    cities: bool,
    capital_cities: bool,
    countries: bool,
    categories: bool,
    #[cfg(feature = "postal")]
    addresses: bool,
}

impl TaggerQueryBuilder {
    pub fn build() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            brands: true,
            cities: true,
            capital_cities: true,
            countries: true,
            categories: true,
            #[cfg(feature = "postal")]
            addresses: true,
        }
    }

    pub fn with_brand(mut self) -> Self {
        self.brands = true;
        self
    }

    pub fn with_cities(mut self) -> Self {
        self.cities = true;
        self
    }

    pub fn with_capital_cities(mut self) -> Self {
        self.capital_cities = true;
        self
    }

    pub fn with_countries(mut self) -> Self {
        self.countries = true;
        self
    }

    pub fn with_categories(mut self) -> Self {
        self.categories = true;
        self
    }

    #[cfg(feature = "postal")]
    pub fn with_addresses(mut self) -> Self {
        self.addresses = true;
        self
    }

    /// Tokenize the input query and apply available taggers
    pub fn apply_taggers(&self, input: &str) -> Vec<TaggedPart> {
        let tokenizer = Tokenizer::parse(input);
        let mut tagged_parts: Vec<TaggedPart> = vec![];
        let mut tagged: Vec<bool> = (0..tokenizer.tokens.len()).map(|_| false).collect();

        for ngram_size in (1..tokenizer.len() + 1).rev() {
            for tokenized in tokenizer.ngrams(ngram_size) {
                if tagged.get(tokenized.span.start + 1) == Some(&true)
                    || tagged[tokenized.span.end - 1]
                {
                    continue;
                }
                let normalized = tokenized.normalize();
                let normalized_token = normalized.as_str();
                let tolerance = (normalized_token.len() >= MAX_DISTANCE_FOR_LEN.1)
                    .then_some(MAX_DISTANCE_FOR_LEN.0);

                if self.brands && BRAND_TAGGER.tag(normalized_token, tolerance) {
                    mark_tagged(&mut tagged, &tokenized);
                    tagged_parts.push(TaggedPart {
                        span: tokenized.span,
                        tag: Tag::Brand,
                        phrase: tokenized.normalize(),
                    });

                    continue;
                }

                if self.capital_cities && CAPITAL_CITY_TAGGER.tag(normalized_token, tolerance) {
                    mark_tagged(&mut tagged, &tokenized);
                    tagged_parts.push(TaggedPart {
                        span: tokenized.span,
                        tag: Tag::City,
                        phrase: tokenized.normalize(),
                    });

                    continue;
                }

                if self.cities && CITY_TAGGER.tag(normalized_token, tolerance) {
                    mark_tagged(&mut tagged, &tokenized);
                    tagged_parts.push(TaggedPart {
                        span: tokenized.span,
                        tag: Tag::City,
                        phrase: tokenized.normalize(),
                    });

                    continue;
                }

                if self.countries && COUNTRIES_TAGGER.tag(normalized_token, tolerance) {
                    mark_tagged(&mut tagged, &tokenized);
                    tagged_parts.push(TaggedPart {
                        span: tokenized.span,
                        tag: Tag::Country,
                        phrase: tokenized.normalize(),
                    });

                    continue;
                }

                if self.categories {
                    if let Some(category) = CATEGORY_TAGGER.tag(normalized_token, tolerance) {
                        mark_tagged(&mut tagged, &tokenized);
                        tagged_parts.push(TaggedPart {
                            span: tokenized.span,
                            tag: Tag::Category(category),
                            phrase: tokenized.normalize(),
                        });

                        continue;
                    }
                }

                #[cfg(feature = "postal")]
                if self.addresses {
                    if let Ok(Some(tag)) = ADDRESS_TAGGER.tag(normalized_token, tolerance) {
                        mark_tagged(&mut tagged, &tokenized);
                        tagged_parts.push(TaggedPart {
                            span: tokenized.span,
                            tag: match tag {
                                AddressTag::Street => Tag::Street,
                                AddressTag::Address => Tag::Address,
                            },
                            phrase: tokenized.normalize(),
                        });

                        continue;
                    }
                }
            }
        }

        fill_untagged_query_parts(&tokenizer, &mut tagged_parts, &tagged);

        tagged_parts.sort_by(|part, other| part.span.start.cmp(&other.span.start));
        tagged_parts
    }
}

// Finalize tagging by adding Tag::None to untagged sections
fn fill_untagged_query_parts(
    tokenizer: &Tokenizer,
    mut tagged_parts: &mut Vec<TaggedPart>,
    tagged: &[bool],
) {
    let mut untagged_start = None;
    let mut untagged_end = None;

    for (idx, has_tag) in tagged.iter().enumerate() {
        if *has_tag || idx == tagged.len() {
            fill_untagged_region(tokenizer, &mut tagged_parts, untagged_start, untagged_end);
            untagged_start = None;
            untagged_end = None;
            continue;
        } else if untagged_start.is_none() {
            untagged_start = Some(idx);
        } else {
            untagged_end = Some(idx + 1);
        }
    }

    fill_untagged_region(tokenizer, &mut tagged_parts, untagged_start, untagged_end);
}

#[inline]
fn fill_untagged_region(
    tokenizer: &Tokenizer,
    tagged_parts: &mut &mut Vec<TaggedPart>,
    start: Option<usize>,
    end: Option<usize>,
) {
    if let (Some(start), Some(end)) = (start, end) {
        let span = Span { start, end };
        tagged_parts.push(TaggedPart {
            span,
            tag: Tag::None,
            phrase: tokenizer.region(span),
        })
    };
}

// Track tagged result to avoid overlapping tags.
// For instance the following tokens ["Campus",  "Paris", "Saclay"] could have both locality "Paris"
// and administrative tag "Campus Paris-Saclay"
fn mark_tagged(tagged: &mut [bool], tokenized: &Tokenized) {
    for has_tag in tagged
        .iter_mut()
        .take(tokenized.span.end)
        .skip(tokenized.span.start)
    {
        *has_tag = true
    }
}

#[cfg(test)]
mod test {
    use crate::tagger::{Tag, TaggedPart};
    use crate::tokens::Span;
    use crate::TaggerQueryBuilder;

    #[test]
    fn brand_with_accent() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_brand()
                .apply_taggers("gamm vért"),
            vec![TaggedPart {
                span: Span { start: 0, end: 2 },
                tag: Tag::Brand,
                phrase: "gamm vert".to_string(),
            }]
        );
    }

    #[test]
    fn multiple_brands() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_brand()
                .apply_taggers("apple nike"),
            vec![
                TaggedPart {
                    span: Span { start: 0, end: 1 },
                    tag: Tag::Brand,
                    phrase: "apple".to_string(),
                },
                TaggedPart {
                    span: Span { start: 1, end: 2 },
                    tag: Tag::Brand,
                    phrase: "nike".to_string(),
                },
            ]
        );
    }

    #[test]
    fn no_brand() {
        assert_eq!(TaggerQueryBuilder::all().apply_taggers("azddaz"), vec![]);
    }

    #[test]
    fn brand_with_remain() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_brand()
                .apply_taggers("apple c'est une pomme"),
            vec![
                TaggedPart {
                    span: Span { start: 0, end: 1 },
                    tag: Tag::Brand,
                    phrase: "apple".to_string(),
                },
                TaggedPart {
                    span: Span { start: 1, end: 5 },
                    tag: Tag::None,
                    phrase: "c est une pomme".to_string(),
                },
            ]
        );
    }

    #[test]
    fn category_tagger_works() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_categories()
                .apply_taggers("restau chinois"),
            vec![TaggedPart {
                span: Span { start: 0, end: 2 },
                tag: Tag::Category("food_chinese".to_string()),
                phrase: "restau chinois".to_string(),
            }]
        );
    }

    #[test]
    fn mixed_tag_works() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_categories()
                .with_brand()
                .apply_taggers("magasin apple"),
            vec![
                TaggedPart {
                    span: Span { start: 0, end: 1 },
                    tag: Tag::Category("shop_supermarket".to_string()),
                    phrase: "magasin".to_string(),
                },
                TaggedPart {
                    span: Span { start: 1, end: 2 },
                    tag: Tag::Brand,
                    phrase: "apple".to_string(),
                },
            ]
        );
    }

    #[test]
    fn cities() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_cities()
                .apply_taggers("Pamandzi"),
            vec![TaggedPart {
                span: Span { start: 0, end: 1 },
                tag: Tag::City,
                phrase: "pamandzi".to_string(),
            },]
        );
    }

    #[test]
    fn apostrophe() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_brand()
                .apply_taggers("L'Atelier"),
            vec![TaggedPart {
                span: Span { start: 0, end: 2 },
                tag: Tag::Brand,
                phrase: "l atelier".to_string(),
            },]
        );
    }

    #[test]
    #[cfg(feature = "postal")]
    fn double_dash() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_addresses()
                .apply_taggers("Franconville-la-garenne"),
            vec![TaggedPart {
                span: Span { start: 0, end: 3 },
                tag: Tag::City,
                phrase: "franconville la garenne".to_string(),
            },]
        );
    }

    #[test]
    #[cfg(feature = "postal")]
    fn address() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_addresses()
                .apply_taggers("156BIS Route de Dijon Brazey-en-Plaine"),
            vec![TaggedPart {
                span: Span { start: 0, end: 7 },
                tag: Tag::Address,
                phrase: "156bis route de dijon brazey en plaine".to_string(),
            },]
        );

        assert_eq!(
            apply_taggers("Route de Dijon Brazey-en-Plaine"),
            vec![TaggedPart {
                span: Span { start: 0, end: 6 },
                tag: Tag::Street,
                phrase: "route de dijon brazey en plaine".to_string(),
            },]
        );
    }

    #[test]
    fn mixed_with_remain() {
        assert_eq!(
            TaggerQueryBuilder::all().apply_taggers("apple c'est une pomme, paris c'est la joie"),
            vec![
                TaggedPart {
                    span: Span { start: 0, end: 1 },
                    tag: Tag::Brand,
                    phrase: "apple".to_string(),
                },
                TaggedPart {
                    span: Span { start: 1, end: 5 },
                    tag: Tag::None,
                    phrase: "c est une pomme".to_string(),
                },
                TaggedPart {
                    span: Span { start: 5, end: 6 },
                    tag: Tag::City,
                    phrase: "paris".to_string(),
                },
                TaggedPart {
                    span: Span { start: 6, end: 10 },
                    tag: Tag::None,
                    phrase: "c est la joie".to_string(),
                },
            ]
        );
    }

    #[test]
    fn fill_untagged_query_parts_partial() {
        assert_eq!(
            TaggerQueryBuilder::build()
                .with_brand()
                .with_categories()
                .apply_taggers("Dior paris toto"),
            vec![
                TaggedPart {
                    span: Span { start: 0, end: 1 },
                    tag: Tag::Brand,
                    phrase: "dior".to_string(),
                },
                TaggedPart {
                    span: Span { start: 1, end: 3 },
                    tag: Tag::None,
                    phrase: "paris toto".to_string(),
                },
            ]
        );
    }
}
