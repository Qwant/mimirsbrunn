use std::marker::PhantomData;

use crate::domain::{
    model::{
        configuration::ContainerConfig, error::Error as ModelError, index::Index,
        update::UpdateOperation,
    },
    ports::secondary::storage::Storage,
};
use async_trait::async_trait;
use common::document::ContainerDocument;
use futures::stream::Stream;
use tracing::{info, info_span};
use tracing_futures::Instrument;

#[async_trait(?Send)]
pub trait GenerateIndex<'s>
where
    Self: Storage<'s> + Send + Sync + 'static,
{
    /// Generate an index and provide an handle over it.
    async fn init_container<'a, D>(
        &'a self,
        config: &'a ContainerConfig,
    ) -> Result<ContainerGenerator<'a, D, Self>, ModelError>
    where
        D: ContainerDocument + Send + Sync + 'static;

    /// Generate an index with provided stream of documents and publish it.
    async fn generate_index<D, S>(
        &'s self,
        config: &'s ContainerConfig,
        documents: S,
    ) -> Result<Index, ModelError>
    where
        D: ContainerDocument + Send + Sync + 'static,
        S: Stream<Item = D> + 's;
}

#[async_trait(?Send)]
impl<'s, T> GenerateIndex<'s> for T
where
    T: Storage<'s> + Send + Sync + Sized + 'static,
{
    #[tracing::instrument(skip(self, config))]
    async fn init_container<'a, D>(
        &'a self,
        config: &'a ContainerConfig,
    ) -> Result<ContainerGenerator<'a, D, Self>, ModelError>
    where
        D: ContainerDocument + Send + Sync + 'static,
    {
        let index = self
            .create_container(config)
            .instrument(info_span!("Create container"))
            .await
            .map_err(|err| ModelError::IndexCreation { source: err.into() })?;

        info!("Created new index: {:?}", index);

        Ok(ContainerGenerator {
            storage: self,
            config,
            index,
            _phantom: PhantomData::default(),
        })
    }

    async fn generate_index<D, S>(
        &'s self,
        config: &'s ContainerConfig,
        documents: S,
    ) -> Result<Index, ModelError>
    where
        D: ContainerDocument + Send + Sync + 'static,
        S: Stream<Item = D> + 's,
    {
        self.init_container(config)
            .await?
            .insert_documents(documents)
            .await?
            .publish()
            .await
    }
}

/// Handle over an index which is beeing generated, it can be used to insert
/// or update documents.  When all documents are ready, `.publish()` must be
/// called to make the index available.
#[must_use = "An index must be used after its documents are built."]
pub struct ContainerGenerator<'a, D, T: ?Sized>
where
    D: ContainerDocument + Send + Sync + 'static,
{
    storage: &'a T,
    config: &'a ContainerConfig,
    index: Index,
    _phantom: PhantomData<*const D>,
}

impl<'a, 's, D, T> ContainerGenerator<'a, D, T>
where
    D: ContainerDocument + Send + Sync + 'static,
    T: Storage<'s> + Send + Sync,
{
    /// Insert new documents into the index
    #[tracing::instrument(skip(self, documents))]
    pub async fn insert_documents(
        self,
        documents: impl Stream<Item = D> + 's,
    ) -> Result<ContainerGenerator<'a, D, T>, ModelError> {
        let stats = self
            .storage
            .insert_documents(self.index.name.clone(), documents)
            .await
            .map_err(|err| ModelError::DocumentStreamInsertion { source: err.into() })?;

        info!("Insertion stats: {:?}", stats);
        Ok(self)
    }

    /// Update documents that have already been inserted
    #[tracing::instrument(skip(self, updates))]
    pub async fn update_documents(
        self,
        updates: impl Stream<Item = (String, Vec<UpdateOperation>)> + 's,
    ) -> Result<ContainerGenerator<'a, D, T>, ModelError> {
        let stats = self
            .storage
            .update_documents(self.index.name.clone(), updates)
            .await
            .map_err(|err| ModelError::DocumentStreamUpdate { source: err.into() })?;

        info!("Update stats: {:?}", stats);
        Ok(self)
    }

    /// Publish the index, which consumes the handle
    #[tracing::instrument(skip(self))]
    pub async fn publish(self) -> Result<Index, ModelError> {
        self.storage
            .publish_index(self.index.clone(), self.config.visibility)
            .await
            .map_err(|err| ModelError::IndexPublication { source: err.into() })?;

        self.storage
            .find_container(self.index.name.clone())
            .await
            .map_err(|err| ModelError::DocumentStreamInsertion { source: err.into() })?
            .ok_or(ModelError::ExpectedIndex {
                index: self.index.name,
            })
    }
}
