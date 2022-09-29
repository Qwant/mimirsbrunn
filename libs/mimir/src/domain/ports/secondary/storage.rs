use async_trait::async_trait;
use config::Config;
use futures::stream::Stream;
use snafu::Snafu;

use crate::domain::model::{
    configuration::{ContainerConfig, ContainerVisibility},
    index::Index,
    stats::InsertStats,
    update::UpdateOperation,
};
use common::document::Document;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Container Creation Error: {}", source))]
    ContainerCreationError { source: Box<dyn std::error::Error> },

    #[snafu(display("Container Deletion Error: {}", source))]
    ContainerDeletionError { source: Box<dyn std::error::Error> },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearchError { source: Box<dyn std::error::Error> },

    #[snafu(display("Document Insertion Error: {}", source))]
    DocumentInsertionError { source: Box<dyn std::error::Error> },

    #[snafu(display("Document Update Error: {}", source))]
    DocumentUpdateError { source: Box<dyn std::error::Error> },

    #[snafu(display("Index Refresh Error: {}", source))]
    IndexPublicationError { source: Box<dyn std::error::Error> },

    #[snafu(display("Force Merge Error: {}", source))]
    ForceMergeError { source: Box<dyn std::error::Error> },

    #[snafu(display("Template '{}' creation error: {}", template, source))]
    TemplateCreationError {
        template: String,
        source: Box<dyn std::error::Error>,
    },

    #[snafu(display("Unrecognized directive: {}", details))]
    UnrecognizedDirective { details: String },
}

#[async_trait(?Send)]
pub trait Storage<'s> {
    async fn create_container(&self, config: &ContainerConfig) -> Result<Index, Error>;

    async fn delete_container(&self, index: String) -> Result<(), Error>;

    async fn find_container(&self, index: String) -> Result<Option<Index>, Error>;

    async fn insert_documents<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats, Error>
    where
        D: Document + Send + Sync + 'static,
        S: Stream<Item = D> + 's;

    async fn update_documents<S>(&self, index: String, operations: S) -> Result<InsertStats, Error>
    where
        S: Stream<Item = (String, Vec<UpdateOperation>)> + 's;

    async fn publish_index(
        &self,
        index: Index,
        visibility: ContainerVisibility,
    ) -> Result<(), Error>;

    async fn configure(&self, directive: String, config: Config) -> Result<(), Error>;
}

#[async_trait(?Send)]
impl<'s, T: ?Sized> Storage<'s> for Box<T>
where
    T: Storage<'s> + Send + Sync,
{
    async fn create_container(&self, config: &ContainerConfig) -> Result<Index, Error> {
        (**self).create_container(config).await
    }

    async fn delete_container(&self, index: String) -> Result<(), Error> {
        (**self).delete_container(index).await
    }

    async fn find_container(&self, index: String) -> Result<Option<Index>, Error> {
        (**self).find_container(index).await
    }

    async fn insert_documents<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats, Error>
    where
        D: Document + Send + Sync + 'static,
        S: Stream<Item = D> + 's,
    {
        (**self).insert_documents(index, documents).await
    }

    async fn update_documents<S>(&self, index: String, operations: S) -> Result<InsertStats, Error>
    where
        S: Stream<Item = (String, Vec<UpdateOperation>)> + 's,
    {
        (**self).update_documents(index, operations).await
    }

    async fn publish_index(
        &self,
        index: Index,
        visibility: ContainerVisibility,
    ) -> Result<(), Error> {
        (**self).publish_index(index, visibility).await
    }

    async fn configure(&self, directive: String, config: Config) -> Result<(), Error> {
        (**self).configure(directive, config).await
    }
}
