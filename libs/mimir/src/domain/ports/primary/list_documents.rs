use crate::domain::{
    model::error::Error as ModelError,
    ports::secondary::list::{List, Parameters},
};
use async_trait::async_trait;
use common::document::ContainerDocument;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use tracing::info_span;
use tracing_futures::Instrument;

type PinnedStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

#[async_trait]
pub trait ListDocuments<D> {
    async fn list_documents(&self) -> Result<PinnedStream<Result<D, ModelError>>, ModelError>;
}

#[async_trait]
impl<D, T> ListDocuments<D> for T
where
    D: ContainerDocument + Send + Sync + 'static,
    T: List<D> + Send + Sync,
{
    async fn list_documents(&self) -> Result<PinnedStream<Result<D, ModelError>>, ModelError> {
        let params = Parameters {
            doc_type: D::static_doc_type().to_string(),
        };

        let documents = self
            .list_documents(params)
            .await?
            .map(|raw| raw.map_err(|err| ModelError::DocumentRetrievalError { source: err.into() }))
            .instrument(info_span!(
                "List documents",
                doc_type = D::static_doc_type(),
            ));

        Ok(documents.boxed())
    }
}
