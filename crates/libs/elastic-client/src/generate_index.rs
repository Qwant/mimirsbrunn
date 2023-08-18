use std::marker::PhantomData;

use crate::model::configuration::ContainerConfig;
use crate::model::error::Error as ModelError;
use crate::model::index::Index;
use crate::model::stats::InsertStats;
use crate::model::update::UpdateOperation;
use crate::ElasticsearchStorage;
use futures::stream::Stream;
use places::ContainerDocument;
use tracing::{info, info_span};
use tracing_futures::Instrument;

impl ElasticsearchStorage {
    #[tracing::instrument(skip(self, config))]
    pub async fn init_container<'a, D>(
        &'a self,
        config: &'a ContainerConfig,
    ) -> Result<ContainerGenerator<'a, D>, ModelError>
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
            stats: Default::default(),
            _phantom: PhantomData,
        })
    }

    pub async fn generate_index<D, S>(
        &self,
        config: &ContainerConfig,
        documents: S,
    ) -> Result<Index, ModelError>
    where
        D: ContainerDocument + Send + Sync + 'static,
        S: Stream<Item = D>,
    {
        self.init_container(config)
            .await?
            .insert_documents(documents)
            .await?
            .publish()
            .await
    }
}

/// Handle over an index which is being generated, it can be used to insert
/// or update documents.  When all documents are ready, `.publish()` must be
/// called to make the index available.
#[must_use = "An index must be used after its documents are built."]
pub struct ContainerGenerator<'a, D>
where
    D: ContainerDocument + Send + Sync + 'static,
{
    storage: &'a ElasticsearchStorage,
    config: &'a ContainerConfig,
    stats: InsertStats,
    index: Index,
    _phantom: PhantomData<*const D>,
}

impl<'a, 's, D> ContainerGenerator<'a, D>
where
    D: ContainerDocument + Send + Sync + 'static,
{
    /// Insert new documents into the index
    #[tracing::instrument(skip(self, documents))]
    pub async fn insert_documents(
        mut self,
        documents: impl Stream<Item = D> + 's,
    ) -> Result<ContainerGenerator<'a, D>, ModelError> {
        let stats = self
            .storage
            .insert_documents(self.index.name.clone(), documents)
            .await
            .map_err(|err| ModelError::DocumentStreamInsertion { source: err.into() })?;

        self.stats += stats;
        info!("Insertion stats: {stats:?}, total: {:?}", self.stats);
        Ok(self)
    }

    /// Update documents that have already been inserted
    #[tracing::instrument(skip(self, updates))]
    pub async fn update_documents(
        mut self,
        updates: impl Stream<Item = (String, Vec<UpdateOperation>)> + 's,
    ) -> Result<ContainerGenerator<'a, D>, ModelError> {
        let stats = self
            .storage
            .update_documents(self.index.name.clone(), updates)
            .await
            .map_err(|err| ModelError::DocumentStreamUpdate { source: err.into() })?;

        self.stats += stats;
        info!("Update stats: {stats:?}, total: {:?}", self.stats);
        Ok(self)
    }

    /// Publish the index, which consumes the handle
    #[tracing::instrument(skip(self))]
    pub async fn publish(self) -> Result<Index, ModelError> {
        let doc_count = self.stats.created - self.stats.deleted;

        if doc_count < self.config.min_expected_count {
            return Err(ModelError::NotEnoughDocuments {
                count: doc_count,
                expected: self.config.min_expected_count,
            });
        }

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
