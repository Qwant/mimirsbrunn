use crate::tokens::Span;

#[cfg(feature = "postal")]
pub mod address;
pub mod brand;
pub mod category;
pub mod location;

/// Utility trait to implement tagging logic, not that the Output type can
/// be anything if additional info needs to be conveyed.
pub trait Tagger {
    type Output;
    /// Apply implementor tagging with the given levenshtein distance.
    fn tag(&self, input: &str, tolerance: Option<u32>) -> Self::Output;
}

/// Represent a tagged section of a query
#[derive(Debug, Eq, PartialEq)]
pub struct TaggedPart {
    pub span: Span,
    pub tag: Tag,
    pub phrase: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Tag {
    Category(String),
    Brand,
    City,
    Address,
    Street,
    Location,
    Poi,
    None,
}
