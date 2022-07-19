use std::time::Duration;

use async_trait::async_trait;

use serde::de::DeserializeOwned;
use snafu::Snafu;

use crate::domain::model::query::Query;

// TODO: this trait seems like bloat: it is the exact same interface as primary port
//       search_documents

#[derive(Debug, Clone)]
pub struct Parameters {
    // pub doc_types: Vec<String>,
    pub query: Query,
    pub result_limit: i64,
    pub timeout: Option<Duration>,
    pub es_indices_to_search_in: Vec<String>,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Document Retrieval Error: {}", source))]
    DocumentRetrievalError { source: Box<dyn std::error::Error> },
}

#[async_trait]
pub trait Search {
    async fn search_documents<D: DeserializeOwned + Send + Sync + 'static>(
        &self,
        parameters: Parameters,
    ) -> Result<Vec<D>, Error>;
}
