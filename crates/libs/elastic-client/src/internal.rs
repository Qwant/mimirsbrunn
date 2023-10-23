use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::pin::Pin;
use std::time::Duration;

use elasticsearch::cat::CatIndicesParts;
use elasticsearch::cluster::{ClusterHealthParts, ClusterPutComponentTemplateParts};
use elasticsearch::indices::{
    IndicesCreateParts, IndicesDeleteParts, IndicesForcemergeParts, IndicesGetAliasParts,
    IndicesPutIndexTemplateParts, IndicesRefreshParts,
};
use elasticsearch::ingest::IngestPutPipelineParts;
use elasticsearch::params::TrackTotalHits;
use elasticsearch::{
    BulkOperation, BulkParts, ExplainParts, MgetParts, OpenPointInTimeParts, SearchParts,
};
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use elastic_query_builder::doc_type::{root_doctype, root_doctype_dataset};
use places::{ContainerDocument, Document};

use crate::dto::{ElasticsearchBulkResult, ElasticsearchGetResponse};
use crate::errors::{ElasticClientError, Result};
use crate::future_helper::with_backoff;
use crate::model::configuration;
use crate::model::index::{Index, IndexStatus};
use crate::model::query::Query;
use crate::model::stats::InsertStats as ModelInsertStats;
use crate::model::status::{StorageHealth, Version as StorageVersion};
use crate::settings::ElasticsearchStorageForceMergeConfig;

use super::configuration::{ComponentTemplateConfiguration, IndexTemplateConfiguration};
use super::dto::{
    ElasticsearchBulkResponse, ElasticsearchForcemergeResponse, ElasticsearchSearchResponse,
};
use super::ElasticSearchClient;

impl ElasticSearchClient {
    pub(crate) async fn create_index(
        &self,
        index_name: &str,
        number_of_shards: u64,
        number_of_replicas: u64,
    ) -> Result<()> {
        let response = self
            .client
            .indices()
            .create(IndicesCreateParts::Index(index_name))
            .body(json!({
                "settings": {
                    "number_of_shards": number_of_shards,
                    "number_of_replicas": number_of_replicas
                }
            }))
            .request_timeout(self.config.timeout)
            .wait_for_active_shards(&self.config.wait_for_active_shards.to_string())
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true), "index": String("name"), "shards_acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response.json::<Value>().await?;

            let acknowledged = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(ElasticClientError::IndexCreationFailed(
                    index_name.to_string(),
                ))
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = ElasticClientError::from(exception);
                    Err(err)
                }
                None => Err(ElasticClientError::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(crate) async fn create_component_template(
        &self,
        config: ComponentTemplateConfiguration,
    ) -> Result<()> {
        let template_name = config.name.clone();
        let body = config.into_json_body()?;
        let response = self
            .client
            .cluster()
            .put_component_template(ClusterPutComponentTemplateParts::Name(&template_name))
            .request_timeout(self.config.timeout)
            .body(body)
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // { "acknowledged": true }
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response.json::<Value>().await?;

            let acknowledged = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected JSON object".to_string(),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected 'acknowledged'".to_owned(),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected JSON bool".to_owned(),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(ElasticClientError::TemplateCreationFailed(template_name))
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = ElasticClientError::from(exception);
                    Err(err)
                }
                None => Err(ElasticClientError::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(crate) async fn create_index_template(
        &self,
        config: IndexTemplateConfiguration,
    ) -> Result<()> {
        let template_name = config.name.clone();
        let body = config.into_json_body()?;
        let response = self
            .client
            .indices()
            .put_index_template(IndicesPutIndexTemplateParts::Name(template_name.as_str()))
            .request_timeout(self.config.timeout)
            .body(body)
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // { "acknowledged": true }
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response.json::<Value>().await?;

            let acknowledged = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;
            if acknowledged {
                Ok(())
            } else {
                Err(ElasticClientError::TemplateCreationFailed(template_name))
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = ElasticClientError::from(exception);
                    Err(err)
                }
                None => Err(ElasticClientError::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(crate) async fn delete_index(&self, index: String) -> Result<()> {
        let response = self
            .client
            .indices()
            .delete(IndicesDeleteParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true), "index": String("name"), "shards_acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response.json::<Value>().await?;

            let acknowledged = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON bool"),
                    json: json.clone(),
                })?;

            if acknowledged {
                Ok(())
            } else {
                Err(ElasticClientError::IndexDeletionFailed(index))
            }
        } else {
            let exception = response.exception().await.ok().unwrap();
            match exception {
                Some(exception) => {
                    let err = ElasticClientError::from(exception);
                    Err(err)
                }
                None => Err(ElasticClientError::ElasticsearchFailureWithoutException),
            }
        }
    }

    // FIXME Move msg to impl ElasticsearchStorage.
    pub(crate) async fn find_index(&self, index: String) -> Result<Index> {
        let response = self
            .client
            .cat()
            .indices(CatIndicesParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .format("json")
            .send()
            .await?;

        if response.status_code().is_success() {
            let json = response.json::<Value>().await?;

            let mut indices: Vec<ElasticsearchIndex> = serde_json::from_value(json)?;

            indices
                .pop()
                .map(Index::try_from)
                .ok_or(ElasticClientError::IndexNotFound(index))?
        } else {
            let exception = response.exception().await.ok().unwrap();

            // We need to handle this exception carefully, so that the 'unknown index' does
            // not result in an Error, but rather a Ok(None) to indicate that nothing was found.

            match exception {
                Some(exception) => Err(ElasticClientError::from(exception)),
                None => Err(ElasticClientError::ElasticsearchFailureWithoutException),
            }
        }
    }

    pub(crate) async fn insert_documents_in_index<D, S>(
        &self,
        index: String,
        documents: S,
    ) -> Result<InsertStats>
    where
        D: Document + Send + Sync + 'static,
        S: Stream<Item = D>,
    {
        let stats = self
            .bulk(
                index,
                documents.map(|doc| {
                    let doc_id = doc.id();
                    BulkOperation::index(doc).id(doc_id).into()
                }),
            )
            .await?;

        if stats.deleted != 0 {
            warn!("Unexpectedly deleted documents during insertion");
        }

        Ok(stats)
    }

    pub(crate) async fn update_documents_in_index<D, S>(
        &self,
        index: String,
        updates: S,
    ) -> Result<InsertStats>
    where
        D: Serialize + Send + Sync + 'static,
        S: Stream<Item = (String, D)>,
    {
        let stats = self
            .bulk(
                index,
                updates.map(|(doc_id, operation)| BulkOperation::update(doc_id, operation).into()),
            )
            .await?;

        if stats.deleted != 0 {
            warn!("Unexpectedly deleted documents during insertion");
        }

        Ok(stats)
    }

    async fn bulk<D, S>(&self, index: String, documents: S) -> Result<InsertStats>
    where
        D: Serialize + Send + Sync + 'static,
        S: Stream<Item = BulkOperation<D>>,
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

    async fn bulk_block<D>(self, index: String, chunk: Vec<BulkOperation<D>>) -> Result<InsertStats>
    where
        D: Serialize + Send + Sync + 'static,
    {
        let mut stats = InsertStats::default();

        let resp = with_backoff(
            || async {
                self.client
                    .bulk(BulkParts::Index(index.as_str()))
                    .request_timeout(self.config.timeout)
                    .body(chunk.iter().collect())
                    .send()
                    .await?
                    .error_for_status_code()
            },
            self.config.bulk_backoff.retry,
            self.config.bulk_backoff.wait,
        )
        .await?;

        if !resp.status_code().is_success() {
            let err = resp
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        } else {
            let es_response: ElasticsearchBulkResponse = resp.json().await?;

            es_response.items.into_iter().try_for_each(|item| {
                let inner = item.inner();
                let result =
                    inner
                        .result
                        .map_err(|err| ElasticClientError::BulkObjectCreationFailed {
                            object_id: inner.id,
                            inner: Box::new(err),
                        })?;

                match result {
                    ElasticsearchBulkResult::Created => stats.created += 1,
                    ElasticsearchBulkResult::Updated => stats.updated += 1,
                    ElasticsearchBulkResult::NoOp => stats.skipped += 1,
                    ElasticsearchBulkResult::Deleted => stats.deleted += 1,
                }

                Ok::<_, ElasticClientError>(())
            })?;

            Ok(stats)
        }
    }

    pub(crate) async fn update_alias(
        &self,
        alias: String,
        indices_to_add: &[String],
        indices_to_remove: &[String],
    ) -> Result<()> {
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
            .and_then(|res| res.error_for_status_code())?;

        let json = response.json::<Value>().await?;

        if json["acknowledged"] == true {
            Ok(())
        } else {
            Err(ElasticClientError::AliasUpdateFailed(alias))
        }
    }

    pub(crate) async fn find_aliases(
        &self,
        index: String,
    ) -> Result<BTreeMap<String, Vec<String>>> {
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
            .await?;

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
            let json = response.json::<Value>().await?;

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
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub(crate) async fn add_pipeline(&self, pipeline: &str, name: &str) -> Result<()> {
        let pipeline: serde_json::Value = serde_json::from_str(pipeline)?;

        let response = self
            .client
            .ingest()
            .put_pipeline(IngestPutPipelineParts::Id(name))
            .request_timeout(self.config.timeout)
            .body(pipeline)
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"acknowledged": Bool(true)})
            // We verify that acknowledge is true, then add the cat indices API to get the full index.
            let json = response.json::<Value>().await?;

            let acknowledged = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("acknowledged")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'acknowledged'"),
                    json: json.clone(),
                })?
                .as_bool()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON boolean"),
                    json: json.clone(),
                })?;

            if acknowledged {
                Ok(())
            } else {
                Err(ElasticClientError::PipelineCreationFailed(name.to_string()))
            }
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub(crate) async fn force_merge(
        &self,
        indices: &[&str],
        config: &ElasticsearchStorageForceMergeConfig,
    ) -> Result<()> {
        let indices_client = self.client.indices();

        let request = indices_client
            .forcemerge(IndicesForcemergeParts::Index(indices))
            .request_timeout(config.timeout);

        let request = {
            if let Some(max_num_segments) = config.max_number_segments {
                request.max_num_segments(max_num_segments)
            } else {
                request
            }
        };

        let response = request.send().await;

        // The forcemerge operation can be very long if a large number of segments have to be
        // merged, in such a case the user may set `allow_timeout` to true in order to let the
        // operation run in background.
        if config.allow_timeout && matches!(&response, Err(err) if err.is_timeout()) {
            warn!(
                "forcemerge query timeout after {:?} on indices {}, it will continue running in background",
                config.timeout,
                indices.join(", "),
            );
            return Ok(());
        }

        let response = response
            .and_then(|res| res.error_for_status_code())?
            .json::<ElasticsearchForcemergeResponse>()
            .await?;

        if response.shards.failed == 0 {
            Ok(())
        } else {
            Err(ElasticClientError::ForceMergeFailed {
                shard_total: response.shards.total,
                shard_failed: response.shards.failed,
                indices: indices.join(","),
            })
        }
    }

    pub(crate) async fn get_previous_indices(&self, index: &Index) -> Result<Vec<String>> {
        let base_index = root_doctype_dataset(&index.root, &index.doc_type, &index.dataset);

        // FIXME When available, we can use aliases.into_keys
        let aliases = self.find_aliases(base_index).await?;
        Ok(aliases
            .into_keys()
            .filter(|i| i.as_str() != index.name)
            .collect())
    }

    pub(crate) async fn refresh_index(&self, index: String) -> Result<()> {
        let response = self
            .client
            .indices()
            .refresh(IndicesRefreshParts::Index(&[&index]))
            .request_timeout(self.config.timeout)
            .send()
            .await?;

        // Note We won't analyze the msg of the response.
        if !response.status_code().is_success() {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        } else {
            Ok(())
        }
    }

    pub async fn list_documents<D>(&self) -> Result<Pin<Box<dyn Stream<Item = Result<D>> + Send>>>
    where
        D: ContainerDocument + Send + Sync,
    {
        let index = root_doctype(&self.config.index_root, D::static_doc_type());
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
                .await?
                .error_for_status_code()?;

            response.json::<PitResponse>().await?.id
        };

        let stream = stream::try_unfold(State::Start, move |state| {
            let client = client.clone();
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
                        .await?;

                    let body: ElasticsearchSearchResponse<D> = response.json().await?;

                    let pit = body
                        .pit_id
                        .clone()
                        .ok_or(ElasticClientError::ElasticsearchResponseMissingPIT)?;

                    let res_status = {
                        if let Some(last_hit) = body.hits.hits.last() {
                            let tiebreaker = last_hit.sort.get(0).unwrap().as_u64().unwrap();
                            info!("Number of documents to retrieve: {} ", tiebreaker);
                            State::Next(ContinuationToken { pit, tiebreaker })
                        } else {
                            State::End(pit)
                        }
                    };

                    let docs = stream::iter(body.into_hits().map(Ok));
                    Ok::<_, ElasticClientError>(Some((docs, res_status)))
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

    pub async fn search_documents<D>(
        &self,
        indices: Vec<String>,
        query: Query,
        limit_result: i64,
        timeout: Option<Duration>,
    ) -> Result<Vec<D>>
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
        let shard_timeout = format!("{}ms", timeout.as_millis());
        let request_timeout = timeout.saturating_add(timeout);

        let search = self
            .client
            .search(SearchParts::Index(&indices))
            // we don't care for the total number of hits, and it takes some time to compute
            // so we disable it
            .track_total_hits(TrackTotalHits::Track(false))
            // global search will end when limit_result are found
            .size(limit_result)
            // search in each *shard* will end after shard_timeout
            .timeout(&shard_timeout)
            // response will be a 408 REQUEST TIMEOUT
            // if I did not receive a full http response from elasticsearch
            // after request_timeout
            .request_timeout(request_timeout)
            // search in each *shard* will end after shard_limit_result hits are found
            // we do not active it, since it means that we may not find the right hit.
            // We did some test when looking for a specific address : we could not 
            // obtain the right address even with shard_limit_result = 10_000
            //.terminate_after(shard_limit_result)
            ;

        let response = match query {
            Query::QueryString(q) => search.q(&q).send().await?,
            Query::QueryDSL(json) => search.body(json).send().await?,
        };

        if response.status_code().is_success() {
            let body = response.json::<ElasticsearchSearchResponse<D>>().await?;

            Ok(body.into_hits().collect())
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub async fn get_documents_by_id<D>(
        &self,
        query: Query,
        timeout: Option<Duration>,
    ) -> Result<Vec<D>>
    where
        D: DeserializeOwned + Send + Sync + 'static,
    {
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

        let get = self.client.mget(MgetParts::None).request_timeout(timeout);

        let response = match query {
            Query::QueryString(_) => {
                return Err(ElasticClientError::QueryStringNotSupported);
            }
            Query::QueryDSL(json) => get.body(json).send().await?,
        };

        if response.status_code().is_success() {
            let body = response.json::<ElasticsearchGetResponse<D>>().await?;

            Ok(body.into_docs().collect())
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub async fn explain_search(
        &self,
        query: Query,
        id: String,
        doc_type: String,
    ) -> Result<serde_json::Value> {
        let index = root_doctype(&self.config.index_root, &doc_type);
        let explain = self
            .client
            .explain(ExplainParts::IndexId(&index, &id))
            .request_timeout(self.config.timeout);

        let response = match query {
            Query::QueryString(q) => explain.q(&q).send().await?,
            Query::QueryDSL(json) => explain.body(json).send().await?,
        };

        if response.status_code().is_success() {
            let json = response.json::<Value>().await?;

            let explanation = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("explanation")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'hits'"),
                    json: json.clone(),
                })?
                .to_owned();

            Ok(explanation)
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub(crate) async fn cluster_health(&self) -> Result<StorageHealth> {
        let response = self
            .client
            .cluster()
            .health(ClusterHealthParts::None)
            .request_timeout(self.config.timeout)
            .send()
            .await?;

        if response.status_code().is_success() {
            // Response similar to:
            // Object({"cluster_name": "foo", "status": "yellow", ...})
            let json = response.json::<Value>().await?;

            let health = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON object"),
                    json: json.clone(),
                })?
                .get("status")
                .ok_or_else(|| ElasticClientError::InvalidJson {
                    msg: String::from("expected 'status'"),
                    json: json.clone(),
                })?
                .as_str()
                .ok_or_else(|| ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON string"),
                    json: json.clone(),
                })?;

            StorageHealth::try_from(health)
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
        }
    }

    pub(crate) async fn cluster_version(&self) -> Result<StorageVersion> {
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
            .await?;

        if response.status_code().is_success() {
            let json = response.json::<Value>().await?;

            let version = json
                .as_array()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON array"),
                    json: json.clone(),
                })?
                .get(0)
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("empty list of node information"),
                    json: json.clone(),
                })?
                .get("v")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'v' (version)"),
                    json: json.clone(),
                })?
                .as_str()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON string"),
                    json: json.clone(),
                })?;
            Ok(version.to_string())
        } else {
            let err = response
                .exception()
                .await?
                .map(ElasticClientError::from)
                .unwrap_or(ElasticClientError::ElasticsearchFailureWithoutException);

            Err(err)
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
    type Error = ElasticClientError;
    fn try_from(index: ElasticsearchIndex) -> Result<Self> {
        let ElasticsearchIndex {
            name,
            docs_count,
            status,
            ..
        } = index;

        let (root, doc_type, dataset) = configuration::split_index_name(&name)?;

        let root = root.to_string();
        let doc_type = doc_type.to_string();
        let dataset = dataset.to_string();

        let docs_count = match docs_count {
            Some(val) => val.parse::<u32>().expect("docs count"),
            None => 0,
        };

        Ok(Index {
            name,
            root,
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
    pub(crate) skipped: usize,
    pub(crate) deleted: usize,
}

impl std::ops::Add for InsertStats {
    type Output = InsertStats;

    fn add(self, rhs: Self) -> Self {
        Self {
            created: self.created + rhs.created,
            updated: self.updated + rhs.updated,
            skipped: self.skipped + rhs.skipped,
            deleted: self.deleted + rhs.deleted,
        }
    }
}

impl From<InsertStats> for ModelInsertStats {
    fn from(stats: InsertStats) -> Self {
        let InsertStats {
            created,
            updated,
            skipped,
            deleted,
        } = stats;

        ModelInsertStats {
            created,
            updated,
            skipped,
            deleted,
        }
    }
}

impl<'a> TryFrom<&'a str> for StorageHealth {
    type Error = ElasticClientError;
    fn try_from(value: &'a str) -> Result<Self> {
        match value {
            "green" | "yellow" => Ok(StorageHealth::OK),
            "red" => Ok(StorageHealth::FAIL),
            _ => Err(ElasticClientError::UnknownElasticSearchStatus(
                value.to_string(),
            )),
        }
    }
}
