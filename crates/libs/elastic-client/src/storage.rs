use config::Config;
use futures::future::TryFutureExt;
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use serde_json::json;
use tracing::info;

use elastic_query_builder::doc_type::{
    root_doctype, root_doctype_dataset, root_doctype_dataset_ts,
};
use places::Document;

use crate::errors::ElasticClientError;
use crate::errors::Result;
use crate::model::configuration::{ContainerConfig, ContainerVisibility};

use super::configuration::{ComponentTemplateConfiguration, IndexTemplateConfiguration};
use super::model::index::Index;
use super::model::stats::InsertStats;
use super::model::update::{generate_document_parts, UpdateOperation};
use super::ElasticsearchStorage;

impl ElasticsearchStorage {
    // This function delegates to elasticsearch the creation of the index. But since this
    // function returns nothing, we follow with a find index to return some details to the caller.
    pub(crate) async fn create_container(&self, config: &ContainerConfig) -> Result<Index> {
        let index_name =
            root_doctype_dataset_ts(&self.config.index_root, &config.name, &config.dataset);

        self.create_index(
            &index_name.clone(),
            config.number_of_shards,
            config.number_of_replicas,
        )
        .and_then(|_| self.find_index(index_name.clone()))
        .await
    }

    pub async fn delete_container(&self, index: String) -> Result<()> {
        self.delete_index(index.clone()).await
    }

    pub async fn find_container(&self, index: String) -> Result<Index> {
        self.find_index(index).await
    }

    // FIXME Explain why we call add_pipeline
    pub(crate) async fn insert_documents<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats>
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
        .await?;

        let insert_stats = self
            .insert_documents_in_index(index.clone(), documents)
            .await
            .map(InsertStats::from)?;

        Ok(insert_stats)
    }

    pub(crate) async fn update_documents<S>(
        &self,
        index: String,
        operations: S,
    ) -> Result<InsertStats>
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
    }

    // FIXME all this should be run in some kind of transaction.
    pub(crate) async fn publish_index(
        &self,
        index: Index,
        visibility: ContainerVisibility,
    ) -> Result<()> {
        if self.config.force_merge.refresh {
            info!("execute 'refresh' on index '{}'", index.name);
            self.refresh_index(index.name.clone()).await?;
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
                .await?;
        }

        let previous_indices = self.get_previous_indices(&index).await?;

        let doctype_dataset_alias =
            root_doctype_dataset(&self.config.index_root, &index.doc_type, &index.dataset);

        self.update_alias(
            doctype_dataset_alias,
            &[index.name.clone()],
            &previous_indices,
        )
        .await?;

        if visibility == ContainerVisibility::Public {
            let doctype_alias = root_doctype(&self.config.index_root, &index.doc_type);

            self.update_alias(
                doctype_alias.clone(),
                &[index.name.clone()],
                &previous_indices,
            )
            .await?;

            self.update_alias(
                self.config.index_root.to_string(),
                &[index.name.clone()],
                &previous_indices,
            )
            .await?;
        }

        for index_name in previous_indices {
            self.delete_container(index_name).await?;
        }

        Ok(())
    }

    pub async fn configure(&self, directive: String, config: Config) -> Result<()> {
        match directive.as_str() {
            "create component template" => {
                // We build a struct from the config object,
                let config = ComponentTemplateConfiguration::new_from_config(config)?;
                self.create_component_template(config).await
            }
            "create index template" => {
                let config = IndexTemplateConfiguration::new_from_config(config)?;
                self.create_index_template(config).await
            }
            _ => Err(ElasticClientError::InvalidDirective(directive)),
        }
    }
}
