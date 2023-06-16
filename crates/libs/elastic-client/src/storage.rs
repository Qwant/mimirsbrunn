use config::Config;
use futures::future::TryFutureExt;
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use serde_json::json;
use tracing::info;

use super::configuration::{ComponentTemplateConfiguration, IndexTemplateConfiguration};
use super::model::index::Index;
use super::model::stats::InsertStats;
use super::model::update::{generate_document_parts, UpdateOperation};
use super::{internal, ElasticsearchStorage};
use crate::model::configuration::{ContainerConfig, ContainerVisibility};
use elastic_query_builder::doc_type::{
    root_doctype, root_doctype_dataset, root_doctype_dataset_ts,
};
use places::Document;
use snafu::Snafu;

impl ElasticsearchStorage {
    // This function delegates to elasticsearch the creation of the index. But since this
    // function returns nothing, we follow with a find index to return some details to the caller.
    pub(crate) async fn create_container(&self, config: &ContainerConfig) -> Result<Index, Error> {
        let index_name =
            root_doctype_dataset_ts(&self.config.index_root, &config.name, &config.dataset);

        self.create_index(
            &index_name,
            config.number_of_shards,
            config.number_of_replicas,
        )
        .and_then(|_| {
            self.find_index(index_name.clone()).and_then(|res| {
                futures::future::ready(res.ok_or(internal::Error::ElasticsearchUnknownIndex {
                    index: index_name.to_string(),
                }))
            })
        })
        .await
        .map_err(|err| Error::ContainerCreationError {
            source: Box::new(err),
        })
    }

    async fn delete_container(&self, index: String) -> Result<(), Error> {
        self.delete_index(index.clone())
            .await
            .map_err(|err| Error::ContainerDeletionError {
                source: Box::new(err),
            })
    }

    pub async fn find_container(&self, index: String) -> Result<Option<Index>, Error> {
        self.find_index(index)
            .await
            .map_err(|err| Error::ContainerSearchError {
                source: Box::new(err),
            })
    }

    // FIXME Explain why we call add_pipeline
    pub(crate) async fn insert_documents<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats, Error>
    where
        D: Document + Send + Sync + 'static,
        S: Stream<Item = D>,
    {
        self.add_pipeline(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../config/pipeline/indexed_at.json",
            )),
            "indexed_at",
        )
        .await
        .map_err(|err| Error::DocumentInsertionError { source: err.into() })?;

        let insert_stats = self
            .insert_documents_in_index(index.clone(), documents)
            .await
            .map(InsertStats::from)
            .map_err(|err| Error::DocumentInsertionError {
                source: Box::new(err),
            })?;

        Ok(insert_stats)
    }

    pub(crate) async fn update_documents<S>(
        &self,
        index: String,
        operations: S,
    ) -> Result<InsertStats, Error>
    where
        S: Stream<Item = (String, Vec<UpdateOperation>)>,
    {
        #[derive(Clone, Serialize)]
        #[serde(into = "serde_json::Value")]
        struct EsOperation(Vec<UpdateOperation>);

        #[allow(clippy::from_over_into)]
        impl Into<serde_json::Value> for EsOperation {
            fn into(self) -> serde_json::Value {
                let updated_parts = generate_document_parts(self.0);
                json!({ "doc": updated_parts })
            }
        }

        let operations = operations.map(|(doc_id, op)| (doc_id, EsOperation(op)));

        self.update_documents_in_index(index, operations)
            .await
            .map(InsertStats::from)
            .map_err(|err| Error::DocumentUpdateError {
                source: Box::new(err),
            })
    }

    // FIXME all this should be run in some kind of transaction.
    pub(crate) async fn publish_index(
        &self,
        index: Index,
        visibility: ContainerVisibility,
    ) -> Result<(), Error> {
        if self.config.force_merge.refresh {
            info!("execute 'refresh' on index '{}'", index.name);
            self.refresh_index(index.name.clone())
                .await
                .map_err(|err| Error::IndexPublicationError {
                    source: Box::new(err),
                })?;
        }

        if self.config.force_merge.enabled {
            // A `force_merge` needs an explicit `refresh` since ElastiSearch 7
            // https://www.elastic.co/guide/en/elasticsearch/reference/7.14/breaking-changes-7.0.html#flush-force-merge-no-longer-refresh
            // `refresh` will be executed during `publish_index`.
            // WARN: `force_merge` is interrupted after a timeout
            // configured in 'elasticsearch.force_merge.timeout'
            // Ideally, this timeout is long enough to finish `force_merge`
            // before `refresh` is executed.
            // In ElastiSearch 8, there is a parameter `wait_for_completion`
            // to handle correctly the end of `force_merge`.
            info!("execute 'force_merge' on index '{}'", index.name);
            self.force_merge(&[&index.name], &self.config.force_merge)
                .await
                .map_err(|err| Error::ForceMergeError {
                    source: Box::new(err),
                })?;
        }

        let previous_indices = self.get_previous_indices(&index).await.map_err(|err| {
            Error::IndexPublicationError {
                source: Box::new(err),
            }
        })?;

        let doctype_dataset_alias =
            root_doctype_dataset(&self.config.index_root, &index.doc_type, &index.dataset);

        self.update_alias(
            doctype_dataset_alias,
            &[index.name.clone()],
            &previous_indices,
        )
        .await
        .map_err(|err| Error::IndexPublicationError {
            source: Box::new(err),
        })?;

        if visibility == ContainerVisibility::Public {
            let doctype_alias = root_doctype(&self.config.index_root, &index.doc_type);

            self.update_alias(
                doctype_alias.clone(),
                &[index.name.clone()],
                &previous_indices,
            )
            .await
            .map_err(|err| Error::IndexPublicationError {
                source: Box::new(err),
            })?;

            self.update_alias(
                self.config.index_root.to_string(),
                &[index.name.clone()],
                &previous_indices,
            )
            .await
            .map_err(|err| Error::IndexPublicationError {
                source: Box::new(err),
            })?;
        }

        for index_name in previous_indices {
            self.delete_container(index_name).await?;
        }

        Ok(())
    }

    pub async fn configure(&self, directive: String, config: Config) -> Result<(), Error> {
        match directive.as_str() {
            "create component template" => {
                // We build a struct from the config object,
                let config =
                    ComponentTemplateConfiguration::new_from_config(config).map_err(|err| {
                        Error::TemplateCreationError {
                            template: String::from("NA"),
                            source: Box::new(err),
                        }
                    })?;
                let template = config.name.clone();
                self.create_component_template(config).await.map_err(|err| {
                    Error::TemplateCreationError {
                        template,
                        source: Box::new(err),
                    }
                })
            }
            "create index template" => {
                let config =
                    IndexTemplateConfiguration::new_from_config(config).map_err(|err| {
                        Error::TemplateCreationError {
                            template: String::from("NA"),
                            source: Box::new(err),
                        }
                    })?;
                let template = config.name.clone();
                self.create_index_template(config).await.map_err(|err| {
                    Error::TemplateCreationError {
                        template,
                        source: Box::new(err),
                    }
                })
            }
            _ => Err(Error::UnrecognizedDirective { details: directive }),
        }
    }
}

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
