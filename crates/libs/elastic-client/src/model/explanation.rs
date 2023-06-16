use serde::{Deserialize, Serialize};

// FIXME This structure is very elasticsearch centric...
// I'd have to find a structure, or a trait that could explain other backend, like
// for example Postgresql EXPLAIN maybe
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Explanation {
    /// score assigned by elasticsearch for that item
    pub value: f64,
    /// description of the operation used to obtained `value` from each `details` values.
    pub description: String,
    /// leafs
    pub details: Vec<Explanation>,
}
