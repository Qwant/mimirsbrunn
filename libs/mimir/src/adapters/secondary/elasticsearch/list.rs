use super::ElasticsearchStorage;
use crate::domain::{
    model::configuration::root_doctype,
    ports::secondary::list::{Error, List, Parameters},
};
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde::de::DeserializeOwned;
use std::pin::Pin;

#[async_trait]
impl<D: DeserializeOwned + Send + Sync + 'static> List<D> for ElasticsearchStorage {
    async fn list_documents(
        &self,
        parameters: Parameters,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<D, Error>> + Send + 'static>>, Error> {
        self.list_documents(root_doctype(&self.config.index_root, &parameters.doc_type))
            .await
            .map_err(|err| Error::DocumentRetrievalError { source: err.into() })
            .map(|stream| {
                stream
                    .map(|item| {
                        item.map_err(|err| Error::DocumentRetrievalError { source: err.into() })
                    })
                    .boxed()
            })
    }
}
