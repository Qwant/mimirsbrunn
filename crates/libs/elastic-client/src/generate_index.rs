use std::marker::PhantomData;

use futures::stream::Stream;
use tracing::{info, info_span};
use tracing_futures::Instrument;

use places::ContainerDocument;

use crate::errors::{ElasticClientError, Result};
use crate::model::configuration::ContainerConfig;
use crate::model::index::Index;
use crate::model::stats::InsertStats;
use crate::model::update::UpdateOperation;
use crate::ElasticsearchStorage;

impl ElasticsearchStorage {
    #[tracing::instrument(skip(self, config))]
    pub async fn init_container<'a, D>(
        &'a self,
        config: &'a ContainerConfig,
    ) -> Result<ContainerGenerator<'a, D>>
    where
        D: ContainerDocument + Send + Sync + 'static,
    {
        let index = self
            .create_container(config)
            .instrument(info_span!("Create container"))
            .await?;

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
    ) -> Result<Index>
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
    ) -> Result<ContainerGenerator<'a, D>> {
        let stats = self
            .storage
            .insert_documents(self.index.name.clone(), documents)
            .await?;

        self.stats += stats;
        info!("Insertion stats: {stats:?}, total: {:?}", self.stats);
        Ok(self)
    }

    /// Update documents that have already been inserted
    #[tracing::instrument(skip(self, updates))]
    pub async fn update_documents(
        mut self,
        updates: impl Stream<Item = (String, Vec<UpdateOperation>)> + 's,
    ) -> Result<ContainerGenerator<'a, D>> {
        let stats = self
            .storage
            .update_documents(self.index.name.clone(), updates)
            .await?;

        self.stats += stats;
        info!("Update stats: {stats:?}, total: {:?}", self.stats);
        Ok(self)
    }

    /// Publish the index, which consumes the handle
    #[tracing::instrument(skip(self))]
    pub async fn publish(self) -> Result<Index> {
        let doc_count = self.stats.created - self.stats.deleted;

        if doc_count < self.config.min_expected_count {
            return Err(ElasticClientError::NotEnoughDocument {
                count: doc_count,
                expected: self.config.min_expected_count,
            });
        }

        self.storage
            .publish_index(self.index.clone(), self.config.visibility)
            .await?;

        self.storage.find_container(self.index.name.clone()).await
    }
}
