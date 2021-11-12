use crate::domain::model::{
    configuration::ContainerConfig, error::Error as ModelError, index::Index,
};
use crate::domain::ports::secondary::storage::Storage;
use async_trait::async_trait;
use common::document::ContainerDocument;
use futures::stream::Stream;
use tracing::{info, info_span};
use tracing_futures::Instrument;

#[async_trait]
pub trait GenerateIndex {
    async fn generate_index<D: ContainerDocument + Send + Sync + 'static>(
        &self,
        config: &ContainerConfig,
        documents: impl Stream<Item = D> + Send + Sync + 'static,
    ) -> Result<Index, ModelError>;
}

#[async_trait]
impl<T> GenerateIndex for T
where
    T: Storage + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, config, documents))]
    async fn generate_index<D: ContainerDocument + Send + Sync + 'static>(
        &self,
        config: &ContainerConfig,
        documents: impl Stream<Item = D> + Send + Sync + 'static,
    ) -> Result<Index, ModelError> {
        // 1. We create the index
        // 2. We insert the document stream in that newly created index
        // 3. We publish the index
        // 4. We search for the newly created index to return it.
        let index = self
            .create_container(config)
            .instrument(info_span!("Create container"))
            .await
            .map_err(|err| ModelError::IndexCreation { source: err.into() })?;

        let stats = self
            .insert_documents(index.name.clone(), documents)
            .instrument(info_span!("Insert documents"))
            .await
            .map_err(|err| ModelError::DocumentStreamInsertion { source: err.into() })?;

        info!("Index generation stats: {:?}", stats);

        self.publish_index(index.clone(), config.visibility)
            .instrument(info_span!("Publish index"))
            .await
            .map_err(|err| ModelError::IndexPublication { source: err.into() })?;

        self.find_container(index.name.clone())
            .await
            .map_err(|err| ModelError::DocumentStreamInsertion { source: err.into() })?
            .ok_or(ModelError::ExpectedIndex { index: index.name })
    }
}