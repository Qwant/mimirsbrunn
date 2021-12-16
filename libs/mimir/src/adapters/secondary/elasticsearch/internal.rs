use elasticsearch::cat::CatIndicesParts;
use elasticsearch::cluster::{ClusterHealthParts, ClusterPutComponentTemplateParts};
use elasticsearch::http::response::Exception;
use elasticsearch::indices::{
    IndicesCreateParts, IndicesDeleteParts, IndicesForcemergeParts, IndicesGetAliasParts,
    IndicesPutIndexTemplateParts, IndicesRefreshParts,
};
use elasticsearch::ingest::IngestPutPipelineParts;
use elasticsearch::{BulkOperation, BulkParts, ExplainParts, OpenPointInTimeParts, SearchParts};
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
use lazy_static::lazy_static;
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snafu::{ResultExt, Snafu};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::pin::Pin;
use std::time::Duration;
use tracing::info;

use super::configuration::{
    ComponentTemplateConfiguration, Error as ConfigurationError, IndexTemplateConfiguration,
};
use super::models::{ElasticsearchBulkResponse, ElasticsearchSearchResponse};
use super::ElasticsearchStorage;
use crate::adapters::secondary::elasticsearch::models::ElasticsearchBulkResult;
use crate::domain::model::{
    configuration,
    index::{Index, IndexStatus},
    query::Query,
    stats::InsertStats as ModelInsertStats,
    status::{StorageHealth, Version as StorageVersion},
};
use common::document::Document;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("Invalid Elasticsearch Index Configuration: {} [{}]", source, details))]
    InvalidConfiguration {
        details: String,
        source: config::ConfigError,
    },

    /// Elasticsearch Errorx
    #[snafu(display("Elasticsearch Error: {} [{}]", source, details))]
    ElasticsearchClient {
        details: String,
        source: elasticsearch::Error,
    },

    /// Elasticsearch Not Created
    #[snafu(display("Elasticsearch Response: Not Created: {}", details))]
    NotCreated { details: String },

    /// Elasticsearch Not Deleted
    #[snafu(display("Elasticsearch Response: Not Deleted: {}", details))]
    NotDeleted { details: String },

    /// Elasticsearch Not Acknowledged
    #[snafu(display("Elasticsearch Response: Not Acknowledged: {}", details))]
    NotAcknowledged { details: String },

    /// Elasticsearch Failed
    #[snafu(display("Elasticsearch Response: Failed: {}", details))]
    Failed { details: String },

    /// Elasticsearch Document Insertion Exception
    #[snafu(display("Elasticsearch Failure without Exception"))]
    ElasticsearchFailureWithoutException,

    /// Elasticsearch Unhandled Exception
    #[snafu(display("Elasticsearch Unhandled Exception: {}", details))]
    ElasticsearchUnhandledException { details: String },

    /// Elasticsearch Duplicate Index
    #[snafu(display("Elasticsearch Duplicate Index: {}", index))]
    ElasticsearchDuplicateIndex { index: String },

    /// Elasticsearch Failed To Parse
    #[snafu(display("Elasticsearch Failed to Parse"))]
    ElasticsearchFailedToParse,

    /// Elasticsearch Failed To Parse
    #[snafu(display("Elasticsearch Failed to Parse Mapping of {}: {}", object, reason))]
    ElasticsearchInvalidMapping { object: String, reason: String },

    /// Elasticsearch Unknown Index
    #[snafu(display("Elasticsearch Unknown Index: {}", index))]
    ElasticsearchUnknownIndex { index: String },

    /// Elasticsearch Unknown Setting
    #[snafu(display("Elasticsearch Unknown Setting: {}", setting))]
    ElasticsearchUnknownSetting { setting: String },

    /// Elasticsearch Failed To Parse
    #[snafu(display("Elasticsearch Failed to Parse Index Settings: {}", reason))]
    ElasticsearchInvalidIndexSettings { reason: String },

    /// Elasticsearch Index Conversion
    #[snafu(display("Index Conversion Error: {}", details))]
    IndexConversion { details: String },

    /// Elasticsearch Deserialization Error
    #[snafu(display("JSON Elasticsearch Deserialization Error: {}", source))]
    ElasticsearchDeserialization { source: elasticsearch::Error },

    /// Elasticsearch Deserialization Error
    #[snafu(display("JSON Serde Deserialization Error: {}", source))]
    JsonDeserialization {
        source: serde_json::Error,
        details: String,
    },

    /// Invalid JSON Value
    #[snafu(display("JSON Deserialization Invalid: {} {:?}", details, json))]
    JsonInvalid { details: String, json: Value },

    /// Internal Error
    #[snafu(display("Internal Error: {}", reason))]
    Internal { reason: String },

    /// Elasticsearch Unhandled Status
    #[snafu(display("Elasticsearch Unhandled Status: {}", details))]
    ElasticsearchUnhandledStatus { details: String },

    /// Elasticsearch Response Has Not PIT
    #[snafu(display("Elasticsearch Response is Missing a PIT"))]
    ElasticsearchResponseMissingPIT,

    /// Invalid Template
    #[snafu(display("Invalid Template: {}", details))]
    InvalidTemplate { details: String },

    /// Invalid Configuration
    #[snafu(display("Invalid Configuration: {}", source))]
    InvalidTemplateConfiguration { source: ConfigurationError },
}

impl From<Exception> for Error {
    // This function analyzes the content of an elasticsearch exception,
    // and returns an error, the type of which should mirror the exception's content.
    // There is no clear blueprint for this analysis, it's very much adhoc.
    fn from(exception: Exception) -> Error {
        let root_cause = exception.error().root_cause();
        if root_cause.is_empty() {
            // If there is no root cause, there maybe a reason
            if let Some(reason) = exception.error().reason() {
                Error::ElasticsearchUnhandledException {
                    details: String::from(reason),
                }
            } else {
                Error::ElasticsearchUnhandledException {
                    details: String::from("Unspecified root cause or reason"),
                }
            }
        } else {
            lazy_static! {
                static ref ALREADY_EXISTS: Regex =
                    Regex::new(r"index \[([^\]/]+).*\] already exists").unwrap();
            }
            lazy_static! {
                static ref NOT_FOUND: Regex = Regex::new(r"no such index \[([^\]/]+).*\]").unwrap();
            }
            lazy_static! {
                static ref FAILED_PARSE: Regex = Regex::new(r"failed to parse").unwrap();
            }
            lazy_static! {
                // Example: Failed to parse mapping [_doc]: analyzer [ngram] has not been configured in mappings
                // we extract an 'object', between [], and the reason, behind ':'
                static ref FAILED_PARSE_MAPPING: Regex =
                    Regex::new(r"Failed to parse mapping \[([^\]/]+).*\]: (.*)").unwrap();
            }
            lazy_static! {
                static ref UNKNOWN_SETTING: Regex =
                    Regex::new(r"unknown setting \[([^\]/]+).*\]").unwrap();
            }
            match root_cause[0].reason() {
                Some(reason) => {
                    if let Some(caps) = ALREADY_EXISTS.captures(reason) {
                        let index = String::from(caps.get(1).unwrap().as_str());
                        Error::ElasticsearchDuplicateIndex { index }
                    } else if let Some(caps) = NOT_FOUND.captures(reason) {
                        let index = String::from(caps.get(1).unwrap().as_str());
                        Error::ElasticsearchUnknownIndex { index }
                    } else if let Some(caps) = FAILED_PARSE_MAPPING.captures(reason) {
                        let object = String::from(caps.get(1).unwrap().as_str());
                        let reason = String::from(caps.get(2).unwrap().as_str());
                        Error::ElasticsearchInvalidMapping { object, reason }
                    } else if FAILED_PARSE.is_match(reason) {
                        Error::ElasticsearchFailedToParse
                    } else if let Some(caps) = UNKNOWN_SETTING.captures(reason) {
                        let setting = String::from(caps.get(1).unwrap().as_str());
                        Error::ElasticsearchUnknownSetting { setting }
                    } else {
                        Error::ElasticsearchUnhandledException {
                            details: format!("Unidentified reason: {}", reason),
                        }
                    }
                }
                None => Error::ElasticsearchUnhandledException {
                    details: String::from("Unspecified reason"),
                },
            }
        }
    }
}

impl From<Option<Exception>> for Error {
    fn from(opt_exc: Option<Exception>) -> Self {
        opt_exc
            .map(Into::into)
            .unwrap_or(Error::ElasticsearchFailureWithoutException)
    }
}

impl ElasticsearchStorage {
    pub(super) async fn create_index(&self, index_name: &str) -> Result<(), Error> {
        let response = self
            .client
            .indices()
            .create(IndicesCreateParts::Index(index_name))
            .request_timeout(self.config.timeout)
            .wait_for_active_shards(&self.config.wait_for_active_shards.to_string())
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot create index '{}'", index_name),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true), "index": String("name"), "shards_acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let acknowledged = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(Error::NotCreated {
                    details: format!("index creation {}", index_name),
                })
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = Error::from(exception);
                    Err(err)
                }
                None => Err(Error::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(super) async fn create_component_template(
        &self,
        config: ComponentTemplateConfiguration,
    ) -> Result<(), Error> {
        let template_name = config.name.clone();
        let body = config
            .into_json_body()
            .context(InvalidTemplateConfiguration)?;
        let response = self
            .client
            .cluster()
            .put_component_template(ClusterPutComponentTemplateParts::Name(&template_name))
            .request_timeout(self.config.timeout)
            .body(body)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot create component template '{}'", template_name),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // { "acknowledged": true }
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let acknowledged = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(Error::NotCreated {
                    details: format!("component template creation {}", template_name),
                })
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = Error::from(exception);
                    Err(err)
                }
                None => Err(Error::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(super) async fn create_index_template(
        &self,
        config: IndexTemplateConfiguration,
    ) -> Result<(), Error> {
        let template_name = config.name.clone();
        let body = config
            .into_json_body()
            .context(InvalidTemplateConfiguration)?;
        let response = self
            .client
            .indices()
            .put_index_template(IndicesPutIndexTemplateParts::Name(template_name.as_str()))
            .request_timeout(self.config.timeout)
            .body(body)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot create component template '{}'", template_name),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // { "acknowledged": true }
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let acknowledged = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(Error::NotCreated {
                    details: format!("component template creation {}", template_name),
                })
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = Error::from(exception);
                    Err(err)
                }
                None => Err(Error::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(super) async fn delete_index(&self, index: String) -> Result<(), Error> {
        let response = self
            .client
            .indices()
            .delete(IndicesDeleteParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot find index '{}'", index),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true), "index": String("name"), "shards_acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let acknowledged = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;

            if acknowledged {
                Ok(())
            } else {
                Err(Error::NotDeleted {
                    details: String::from(
                        "Elasticsearch response to index deletion not acknowledged",
                    ),
                })
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = Error::from(exception);
                    Err(err)
                }
                None => Err(Error::ElasticsearchFailureWithoutException),
            }
        }
    }

    // FIXME Move details to impl ElasticsearchStorage.
    pub(super) async fn find_index(&self, index: String) -> Result<Option<Index>, Error> {
        let response = self
            .client
            .cat()
            .indices(CatIndicesParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .format("json")
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot find index '{}'", index),
            })?;

        if response.status_code().is_success() {
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let mut indices: Vec<ElasticsearchIndex> =
                serde_json::from_value(json).context(JsonDeserialization {
                    details: String::from("could not deserialize Elasticsearch indices"),
                })?;

            indices.pop().map(Index::try_from).transpose()
        } else {
            let exception = response.exception().await.ok().unwrap();

            // We need to handle this exception carefully, so that the 'unknown index' does
            // not result in an Error, but rather a Ok(None) to indicate that nothing was found.

            match exception {
                Some(exception) => {
                    let err = Error::from(exception);
                    if std::matches!(err, Error::ElasticsearchUnknownIndex { .. }) {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                }
                None => Err(Error::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(super) async fn insert_documents_in_index<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats, Error>
    where
        D: Document + Send + Sync + 'static,
        S: Stream<Item = D> + Send + Sync,
    {
        self.bulk(
            index,
            documents.map(|doc| {
                let doc_id = doc.id();
                BulkOperation::index(doc).id(doc_id).into()
            }),
        )
        .await
    }

    pub(super) async fn update_documents_in_index<D, S>(
        &self,
        index: String,
        updates: S,
    ) -> Result<InsertStats, Error>
    where
        D: Serialize + Send + Sync + 'static,
        S: Stream<Item = (String, D)> + Send + Sync,
    {
        self.bulk(
            index,
            updates.map(|(doc_id, operation)| BulkOperation::update(doc_id, operation).into()),
        )
        .await
    }

    async fn bulk<D, S>(&self, index: String, documents: S) -> Result<InsertStats, Error>
    where
        D: Serialize + Send + Sync + 'static,
        S: Stream<Item = BulkOperation<D>> + Send + Sync,
    {
        let stats = documents
            .chunks(self.config.insertion_chunk_size)
            .map(|chunk| {
                let index = index.clone();
                let client = self.clone();

                async move {
                    tokio::spawn(client.bulk_block(index, chunk))
                        .await
                        .expect("tokio task panicked")
                        .unwrap_or_else(|err| panic!("Error inserting chunk: {}", err))
                }
            })
            .buffer_unordered(self.config.insertion_concurrent_requests)
            .fold(InsertStats::default(), |acc, loc| async move { acc + loc })
            .await;

        Ok(stats)
    }

    async fn bulk_block<D>(
        self,
        index: String,
        chunk: Vec<BulkOperation<D>>,
    ) -> Result<InsertStats, Error>
    where
        D: Serialize + Send + Sync + 'static,
    {
        let mut stats = InsertStats::default();

        let resp = self
            .client
            .bulk(BulkParts::Index(index.as_str()))
            .request_timeout(self.config.timeout)
            .body(chunk)
            .send()
            .await
            .and_then(|res| res.error_for_status_code())
            .context(ElasticsearchClient {
                details: "cannot bulk insert",
            })?;

        if !resp.status_code().is_success() {
            Err(resp
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        } else {
            let es_response: ElasticsearchBulkResponse =
                resp.json().await.context(ElasticsearchDeserialization)?;

            es_response.items.into_iter().try_for_each(|item| {
                let result = item.inner().result.map_err(|err| Error::NotCreated {
                    details: err.reason,
                })?;

                match result {
                    ElasticsearchBulkResult::Created => stats.created += 1,
                    ElasticsearchBulkResult::Updated => stats.updated += 1,
                    _ => unreachable!("no port implements document deletion"),
                }

                Ok::<_, Error>(())
            })?;

            Ok(stats)
        }
    }

    pub(super) async fn update_alias(
        &self,
        alias: String,
        indices_to_add: &[String],
        indices_to_remove: &[String],
    ) -> Result<(), Error> {
        let mut actions = vec![];

        if !indices_to_add.is_empty() {
            actions.push(json!({
                "add": {
                    "alias": alias,
                    "indices": indices_to_add,
                }
            }));
        };

        if !indices_to_remove.is_empty() {
            actions.push(json!({
                "remove": {
                    "alias": alias,
                    "indices": indices_to_remove,
                }
            }));
        };

        if actions.is_empty() {
            return Ok(());
        }

        let response = self
            .client
            .indices()
            .update_aliases()
            .request_timeout(self.config.timeout)
            .body(json!({ "actions": actions }))
            .send()
            .await
            .and_then(|res| res.error_for_status_code())
            .context(ElasticsearchClient {
                details: format!("cannot update alias '{}'", alias),
            })?;

        let json = response
            .json::<Value>()
            .await
            .context(ElasticsearchDeserialization)?;

        if json["acknowledged"] == true {
            Ok(())
        } else {
            Err(Error::NotAcknowledged {
                details: format!("cannot update alias '{}'", alias),
            })
        }
    }

    pub(super) async fn find_aliases(
        &self,
        index: String,
    ) -> Result<BTreeMap<String, Vec<String>>, Error> {
        // The last piece of the input index should be a dataset
        // If you didn't add the trailing '_*' below, when you would search for
        // the aliases of eg 'fr', you would also find the aliases for 'fr-ne'.
        let index = format!("{}_*", index);
        let response = self
            .client
            .indices()
            .get_alias(IndicesGetAliasParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot find aliases to {}", index),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // {
            //   "index1": {
            //      "aliases": {
            //         "alias1": {},
            //         "alias2": {}
            //      }
            //   },
            //   "index2": {
            //      "aliases": {
            //         "alias3": {}
            //      }
            //   }
            // }
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let aliases = json
                .as_object()
                .map(|indices| {
                    indices
                        .iter()
                        .filter_map(|(index, value)| {
                            value["aliases"]
                                .as_object()
                                .map(|aliases| (index.clone(), aliases.keys().cloned().collect()))
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    info!("No alias for index {}", index);
                    BTreeMap::new()
                });
            Ok(aliases)
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }

    pub(super) async fn add_pipeline(&self, pipeline: &str, name: &str) -> Result<(), Error> {
        let pipeline: serde_json::Value =
            serde_json::from_str(pipeline).context(JsonDeserialization {
                details: format!("Could not deserialize pipeline {}", name),
            })?;

        let response = self
            .client
            .ingest()
            .put_pipeline(IngestPutPipelineParts::Id(name))
            .request_timeout(self.config.timeout)
            .body(pipeline)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot add pipeline '{}'", name,),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let acknowledged = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON boolean"),
                    json: json.clone(),
                })?;

            if acknowledged {
                Ok(())
            } else {
                Err(Error::NotAcknowledged {
                    details: format!("pipeline {} creation", name),
                })
            }
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }

    pub(super) async fn force_merge(
        &self,
        indices: &[&str],
        max_num_segments: i64,
    ) -> Result<(), Error> {
        let response = self
            .client
            .indices()
            .forcemerge(IndicesForcemergeParts::Index(indices))
            .max_num_segments(max_num_segments)
            // .request_timeout(self.config.timeout) This call is not using timeout because
            // it can take a long time and would require the timeout to become very large,
            // and meaningless for other operations.
            .send()
            .await
            .and_then(|res| res.error_for_status_code())
            .context(ElasticsearchClient {
                details: format!(
                    "cannot force merge indices '{}'",
                    indices
                        .iter()
                        .map(|s| &**s)
                        .collect::<Vec<&str>>()
                        .join(", ")
                ),
            })?;

        let json = response
            .json::<Value>()
            .await
            .context(ElasticsearchDeserialization)?;

        if json["_shards"]["successful"] == 1 {
            Ok(())
        } else {
            Err(Error::Failed {
                details: format!(
                    "cannot force merge '{}'",
                    indices
                        .iter()
                        .map(|s| &**s)
                        .collect::<Vec<&str>>()
                        .join(", ")
                ),
            })
        }
    }

    pub(super) async fn get_previous_indices(&self, index: &Index) -> Result<Vec<String>, Error> {
        let base_index = configuration::root_doctype_dataset(&index.doc_type, &index.dataset);
        // FIXME When available, we can use aliases.into_keys
        let aliases = self.find_aliases(base_index).await?;
        Ok(aliases
            .into_iter()
            .map(|(k, _)| k)
            .filter(|i| i.as_str() != index.name)
            .collect())
    }

    pub(super) async fn refresh_index(&self, index: String) -> Result<(), Error> {
        let response = self
            .client
            .indices()
            .refresh(IndicesRefreshParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .send()
            .await
            .context(ElasticsearchClient {
                details: format!("cannot refresh index {}", index),
            })?;

        // Note We won't analyze the details of the response.
        if !response.status_code().is_success() {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        } else {
            Ok(())
        }
    }

    pub(super) async fn list_documents<D>(
        &self,
        index: String,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<D, Error>> + Send>>, Error>
    where
        D: DeserializeOwned + Send + Sync + 'static,
    {
        let client = self.client.clone();
        let timeout = self.config.timeout;
        let chunk_size = self.config.scroll_chunk_size;
        let pit_alive = self.config.scroll_pit_alive.clone();

        // Open initial PIT
        let init_pit = {
            #[derive(Deserialize)]
            struct PitResponse {
                id: String,
            }

            let response = client
                .open_point_in_time(OpenPointInTimeParts::Index(&[&index]))
                .request_timeout(timeout)
                .keep_alive(&pit_alive)
                .send()
                .await
                .context(ElasticsearchClient {
                    details: format!("failed to query PIT for {}", index),
                })?
                .error_for_status_code()
                .context(ElasticsearchClient {
                    details: format!("failed to open PIT for {}", index),
                })?;

            response
                .json::<PitResponse>()
                .await
                .context(ElasticsearchDeserialization)?
                .id
        };

        let stream = stream::try_unfold(State::Start, move |state| {
            let client = client.clone();
            let index = index.clone();
            let init_pit = init_pit.clone();
            let pit_alive = pit_alive.clone();

            // Build the query for the next chunk of documents.
            let build_query = move |pit_id, search_after| {
                let mut query = json!({
                    "query": {"match_all": {}},
                    "size": chunk_size,
                    "pit": {"id": pit_id, "keep_alive": pit_alive},
                    "track_total_hits": false,
                    "sort": [{"_shard_doc": "desc"}]
                });

                if let Some(search_after) = search_after {
                    query["search_after"] = json!([search_after]);
                }

                query
            };

            // Fetch Elasticsearch response, build stream over returned chunk and compute next
            // state.
            let read_response = {
                let client = client.clone();

                move |query| async move {
                    let response = client
                        .search(SearchParts::None)
                        .request_timeout(timeout)
                        .body(query)
                        .send()
                        .await
                        .context(ElasticsearchClient {
                            details: format!("failed to search for {}", index),
                        })?;

                    let body: ElasticsearchSearchResponse<D> = response
                        .json()
                        .await
                        .context(ElasticsearchDeserialization)?;

                    let pit = body
                        .pit_id
                        .clone()
                        .ok_or(Error::ElasticsearchResponseMissingPIT)?;

                    let res_status = {
                        if let Some(last_hit) = body.hits.hits.last() {
                            let tiebreaker = last_hit.sort.get(0).unwrap().as_u64().unwrap();
                            State::Next(ContinuationToken { pit, tiebreaker })
                        } else {
                            State::End(pit)
                        }
                    };

                    let docs = stream::iter(body.into_hits().map(Ok));
                    Ok::<_, Error>(Some((docs, res_status)))
                }
            };

            async move {
                match state {
                    State::Start => {
                        let query = build_query(init_pit, None);
                        read_response(query).await
                    }
                    State::Next(continuation_token) => {
                        let query = build_query(
                            continuation_token.pit,
                            Some(continuation_token.tiebreaker),
                        );

                        read_response(query).await
                    }
                    State::End(pit) => {
                        let response = client
                            .close_point_in_time()
                            .body(json!({ "id": pit }))
                            .send()
                            .await
                            .unwrap();

                        let _response_body = response.json::<Value>().await.unwrap();
                        Ok(None)
                    }
                }
            }
        })
        .try_flatten();

        Ok(stream.boxed())
    }

    pub(super) async fn search_documents<D>(
        &self,
        indices: Vec<String>,
        query: Query,
        limit_result: i64,
        timeout: Option<Duration>,
    ) -> Result<Vec<D>, Error>
    where
        D: DeserializeOwned + Send + Sync + 'static,
    {
        let indices = indices.iter().map(String::as_str).collect::<Vec<_>>();
        let timeout = timeout
            .map(|t| {
                if t > self.config.timeout {
                    info!(
                        "Requested timeout {:?} is too big. I'll use {:?} instead.",
                        t, self.config.timeout
                    );
                    self.config.timeout
                } else {
                    t
                }
            }) // let's cap the timeout to self.config.timeout to prevent overloading elasticsearch with long requests
            .unwrap_or(self.config.timeout);

        let search = self
            .client
            .search(SearchParts::Index(&indices))
            .size(limit_result)
            .request_timeout(timeout);

        let response = match query {
            Query::QueryString(q) => search.q(&q).send().await.context(ElasticsearchClient {
                details: format!("could not search indices {}", indices.join(", ")),
            })?,
            Query::QueryDSL(json) => {
                search
                    .body(json)
                    .send()
                    .await
                    .context(ElasticsearchClient {
                        details: format!("could not search indices {}", indices.join(", ")),
                    })?
            }
        };

        if response.status_code().is_success() {
            let body = response
                .json::<ElasticsearchSearchResponse<D>>()
                .await
                .context(ElasticsearchDeserialization)?;

            Ok(body.into_hits().collect())
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }

    pub(super) async fn explain_search<D>(
        &self,
        index: String,
        query: Query,
        id: String,
    ) -> Result<D, Error>
    where
        D: DeserializeOwned + Send + Sync + 'static,
    {
        let explain = self
            .client
            .explain(ExplainParts::IndexId(&index, &id))
            .request_timeout(self.config.timeout);

        let response = match query {
            Query::QueryString(q) => explain.q(&q).send().await.context(ElasticsearchClient {
                details: format!("could not explain document {} in index {}", id, index),
            })?,
            Query::QueryDSL(json) => {
                explain
                    .body(json)
                    .send()
                    .await
                    .context(ElasticsearchClient {
                        details: format!("could not explain document {} in index {}", id, index),
                    })?
            }
        };

        if response.status_code().is_success() {
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            println!("json: {:?}", json);

            let explanation = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("explanation")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'hits'"),
                    json: json.clone(),
                })?
                .to_owned();
            let explanation =
                serde_json::from_value::<D>(explanation).context(JsonDeserialization {
                    details: String::from("could not deserialize explanation"),
                })?;
            Ok(explanation)
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }

    pub(super) async fn cluster_health(&self) -> Result<StorageHealth, Error> {
        let response = self
            .client
            .cluster()
            .health(ClusterHealthParts::None)
            .request_timeout(self.config.timeout)
            .send()
            .await
            .context(ElasticsearchClient {
                details: String::from("cannot query cluster health"),
            })?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"cluster_name": "foo", "status": "yellow", ...})
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let health = json
                .as_object()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("status")
                .ok_or_else(|| Error::JsonInvalid {
                    details: String::from("expected 'status'"),
                    json: json.clone(),
                })?
                .as_str()
                .ok_or_else(|| Error::JsonInvalid {
                    details: String::from("expected JSON string"),
                    json: json.clone(),
                })?;

            StorageHealth::try_from(health)
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }

    pub(super) async fn cluster_version(&self) -> Result<StorageVersion, Error> {
        // In the following, we specify the list of columns we're interested in ("v" for version).
        // Refer to https://www.elastic.co/guide/en/elasticsearch/reference/current/cat-nodes.html
        // to explicitely set the list of columns
        let response = self
            .client
            .cat()
            .nodes()
            .request_timeout(self.config.timeout)
            .h(&["v"]) // We only want the version
            .format("json")
            .send()
            .await
            .context(ElasticsearchClient {
                details: String::from("cannot query cluster health"),
            })?;

        if response.status_code().is_success() {
            let json = response
                .json::<Value>()
                .await
                .context(ElasticsearchDeserialization)?;

            let version = json
                .as_array()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON array"),
                    json: json.clone(),
                })?
                .get(0)
                .ok_or(Error::JsonInvalid {
                    details: String::from("empty list of node information"),
                    json: json.clone(),
                })?
                .get("v")
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected 'v' (version)"),
                    json: json.clone(),
                })?
                .as_str()
                .ok_or(Error::JsonInvalid {
                    details: String::from("expected JSON string"),
                    json: json.clone(),
                })?;
            Ok(version.to_string())
        } else {
            Err(response
                .exception()
                .await
                .expect("failed to fetch Elasticsearch exception")
                .into())
        }
    }
}

/// This is the information provided by Elasticsearch CAT Indice API
#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct ElasticsearchIndex {
    pub(crate) health: String,
    pub status: String,
    #[serde(rename = "index")]
    pub(crate) name: String,
    #[serde(rename = "docs.count")]
    pub(crate) docs_count: Option<String>,
    #[serde(rename = "docs.deleted")]
    pub(crate) docs_deleted: Option<String>,
    pub(crate) pri: String,
    #[serde(rename = "pri.store.size")]
    pub(crate) pri_store_size: Option<String>,
    pub(crate) rep: String,
    #[serde(rename = "store.size")]
    pub(crate) store_size: Option<String>,
    pub(crate) uuid: String,
}

impl TryFrom<ElasticsearchIndex> for Index {
    type Error = Error;
    fn try_from(index: ElasticsearchIndex) -> Result<Self, Self::Error> {
        let ElasticsearchIndex {
            name,
            docs_count,
            status,
            ..
        } = index;
        let (doc_type, dataset) =
            configuration::split_index_name(&name).map_err(|err| Error::IndexConversion {
                details: format!(
                    "could not convert elasticsearch index into model index: {}",
                    err.to_string()
                ),
            })?;

        let docs_count = match docs_count {
            Some(val) => val.parse::<u32>().expect("docs count"),
            None => 0,
        };
        Ok(Index {
            name,
            doc_type,
            dataset,
            docs_count,
            status: IndexStatus::from(status),
        })
    }
}

impl From<String> for IndexStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "green" => IndexStatus::Available,
            "yellow" => IndexStatus::Available,
            _ => IndexStatus::Available,
        }
    }
}

struct ContinuationToken {
    pit: String,
    tiebreaker: u64,
}

enum State {
    Start,
    Next(ContinuationToken),
    End(String),
}

#[derive(Debug, Default)]
pub struct InsertStats {
    pub(crate) created: usize,
    pub(crate) updated: usize,
}

impl std::ops::Add for InsertStats {
    type Output = InsertStats;

    fn add(self, rhs: Self) -> Self {
        Self {
            created: self.created + rhs.created,
            updated: self.updated + rhs.updated,
        }
    }
}

impl From<InsertStats> for ModelInsertStats {
    fn from(stats: InsertStats) -> Self {
        let InsertStats { created, updated } = stats;
        ModelInsertStats { created, updated }
    }
}

impl<'a> TryFrom<&'a str> for StorageHealth {
    type Error = Error;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "green" | "yellow" => Ok(StorageHealth::OK),
            "red" => Ok(StorageHealth::FAIL),
            _ => Err(Error::ElasticsearchUnhandledStatus {
                details: value.to_string(),
            }),
        }
    }
}
