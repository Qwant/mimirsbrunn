use async_trait::async_trait;
use serde::de::DeserializeOwned;

use super::ElasticsearchStorage;
use crate::domain::ports::secondary::{
    get::{Error as GetError, Get, Parameters as GetParameters},
    search::{Error as SearchError, Parameters as SearchParameters, Search},
};

#[async_trait]
impl Search for ElasticsearchStorage {
    async fn search_documents<D: DeserializeOwned + Send + Sync + 'static>(
        &self,
        parameters: SearchParameters,
    ) -> Result<Vec<D>, SearchError> {
        self.search_documents(
            parameters.es_indices_to_search_in,
            parameters.query,
            parameters.result_limit,
            parameters.timeout,
        )
        .await
        .map_err(|err| SearchError::DocumentRetrievalError { source: err.into() })
    }
}

#[async_trait]
impl Get for ElasticsearchStorage {
    type Doc = serde_json::Value;

    async fn get_documents_by_id(
        &self,
        parameters: GetParameters,
    ) -> Result<Vec<Self::Doc>, GetError> {
        self.get_documents_by_id(parameters.query, parameters.timeout)
            .await
            .map_err(|err| GetError::DocumentRetrievalError { source: err.into() })
    }
}
