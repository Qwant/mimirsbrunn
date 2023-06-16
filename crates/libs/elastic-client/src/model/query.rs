#[derive(Debug, Clone)]
pub enum Query {
    QueryString(String),
    QueryDSL(serde_json::Value),
}
