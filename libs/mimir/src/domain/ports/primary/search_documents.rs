use std::time::Duration;

use async_trait::async_trait;
use serde::de::DeserializeOwned;

use crate::domain::{
    model::{error::Error as ModelError, query::Query},
    ports::secondary::search::{Parameters, Search},
};

// automock requires that the associated type be known at
// compile time. So here we import admins when we are in
// testing configuration.

#[async_trait]
pub trait SearchDocuments {
    async fn search_documents<D: DeserializeOwned + Send + Sync + 'static>(
        &self,
        es_indices_to_search_in: Vec<String>,
        query: Query,
        result_limit: i64,
        timeout: Option<Duration>,
    ) -> Result<Vec<D>, ModelError>;
}

#[async_trait]
impl<T> SearchDocuments for T
where
    T: Search + Send + Sync,
{
    async fn search_documents<D: DeserializeOwned + Send + Sync + 'static>(
        &self,
        es_indices_to_search_in: Vec<String>,
        query: Query,
        result_limit: i64,
        timeout: Option<Duration>,
    ) -> Result<Vec<D>, ModelError> {
        self.search_documents(Parameters {
            es_indices_to_search_in,
            query,
            result_limit,
            timeout,
        })
        .await
        .map_err(|err| ModelError::DocumentRetrievalError { source: err.into() })
    }
}
