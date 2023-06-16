use serde::{Deserialize, Serialize};

/// Contextual information related to the query. It can be used to store information
/// for monitoring performance, search relevance, ...
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Context {
    /// Elasticsearch explanation
    pub explanation: Option<Explanation>,
}

/// This structure is used when analyzing the result of an Elasticsearch 'explanation' query,
/// which describes the construction of the score". It is a tree structure.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Explanation {
    /// score assigned by elasticsearch for that item
    pub value: f64,
    /// description of the operation used to obtained `value` from each `details` values.
    pub description: String,
    /// leafs
    pub details: Vec<Explanation>,
}
