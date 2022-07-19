//! ES response for various ES queries, these only serialize the fields that we use,
/// which can be prone to change in the future
use serde::{Deserialize, Deserializer};
use serde_json::Value;

// Search API
// See https://www.elastic.co/guide/en/elasticsearch/reference/8.1/search-search.html

/// ES response for a search query.
#[derive(Deserialize)]
pub struct ElasticsearchSearchResponse<D> {
    pub pit_id: Option<String>,
    pub hits: ElasticsearchHits<D>,
}

#[derive(Deserialize)]
pub struct ElasticsearchHits<D> {
    pub hits: Vec<ElasticsearchHit<D>>,
}

#[derive(Deserialize)]
pub struct ElasticsearchHit<D> {
    #[serde(rename = "_source")]
    pub source: D,
    #[serde(default)]
    pub sort: Vec<Value>,
}

impl<D> ElasticsearchSearchResponse<D> {
    /// Consume the response into an iterator over the responded documents.
    pub fn into_hits(self) -> impl Iterator<Item = D> {
        self.hits.hits.into_iter().map(|hit| hit.source)
    }
}

// Get API
// See https://www.elastic.co/guide/en/elasticsearch/reference/8.1/docs-get.html

/// ES response for a get query.
#[derive(Deserialize)]
pub struct ElasticsearchGetResponse<D> {
    pub docs: Vec<ElasticsearchDocs<D>>,
}

#[derive(Deserialize)]
pub struct ElasticsearchDocs<D> {
    #[serde(rename = "_source")]
    pub source: Option<D>,
}

impl<D> ElasticsearchGetResponse<D> {
    /// Consume the response into an iterator over the responded documents.
    pub fn into_docs(self) -> impl Iterator<Item = D> {
        self.docs.into_iter().filter_map(|doc| doc.source)
    }
}

// Bulk API (only implemented for index and update)
// See https://www.elastic.co/guide/en/elasticsearch/reference/8.1/docs-bulk.html

/// ES response for bulk insert queries.
#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchBulkResponse {
    pub items: Vec<ElasticsearchBulkItem>,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElasticsearchBulkItem {
    Index(ElasticsearchBulkStatus),
    Update(ElasticsearchBulkStatus),
}

impl ElasticsearchBulkItem {
    pub fn inner(self) -> ElasticsearchBulkStatus {
        match self {
            ElasticsearchBulkItem::Index(inner) => inner,
            ElasticsearchBulkItem::Update(inner) => inner,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchBulkStatus {
    pub status: u16,
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(flatten, deserialize_with = "deserialize_bulk_result")]
    pub result: Result<ElasticsearchBulkResult, ElasticsearchBulkError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElasticsearchBulkResult {
    Created,
    Updated,
    Deleted,
    NoOp,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchBulkErrorCausedBy {
    #[serde(rename = "type")]
    pub caused_by_type: String,
    pub reason: String,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchBulkError {
    #[serde(rename = "type")]
    pub err_type: String,
    pub reason: String,
    pub index: Option<String>,
    pub index_uuid: Option<String>,
    pub shard: Option<String>,
    pub caused_by: Option<ElasticsearchBulkErrorCausedBy>,
}

// Force Merge API
// See https://www.elastic.co/guide/en/elasticsearch/reference/8.1/indices-forcemerge.html

#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchForcemergeResponse {
    #[serde(rename = "_shards")]
    pub shards: ElasticsearchShardsResult,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
pub struct ElasticsearchShardsResult {
    pub successful: u32,
    pub failed: u32,
    pub total: u32,
}

// Custom deserializers

/// Serialize { "result": T, "error": E } into a Result<T, E>
pub fn deserialize_bulk_result<'de, D, T: Deserialize<'de>, E: Deserialize<'de>>(
    deserializer: D,
) -> Result<Result<T, E>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum ElasticsearchResult<T, E> {
        Result(T),
        Error(E),
    }

    Ok({
        match Deserialize::deserialize(deserializer)? {
            ElasticsearchResult::Result(x) => Ok(x),
            ElasticsearchResult::Error(err) => Err(err),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Build a sample response for Bulk API adapted from documentation:
    /// https://www.elastic.co/guide/en/elasticsearch/reference/current/docs-bulk.html#docs-bulk-api-example
    fn sample() -> serde_json::Value {
        json!({
            "took": 486,
            "errors": true,
            "items": [
                {
                    "index": {
                        "_index": "index1",
                        "_type" : "_doc",
                        "_id": "5",
                        "status": 404,
                        "error": {
                            "type": "document_missing_exception",
                            "reason": "[_doc][5]: document missing",
                            "index_uuid": "aAsFqTI0Tc2W0LCWgPNrOA",
                            "shard": "0",
                            "index": "index1"
                        }
                    }
                },
                {
                    "update": {
                        "_index": "index1",
                        "_type" : "_doc",
                        "_id": "7",
                        "_version": 1,
                        "result": "created",
                        "_shards": {
                            "total": 2,
                            "successful": 1,
                            "failed": 0
                        },
                        "_seq_no": 0,
                        "_primary_term": 1,
                        "status": 201
                    }
                }
            ]
        })
    }

    fn sample_400_error() -> serde_json::Value {
        json!({
            "took": 0,
            "errors": true,
            "items": [
                {
                    "index": {
                    "_index": "test_coord",
                    "_type": "_doc",
                    "_id": "StopArea:TCL:01",
                    "status": 400,
                    "error": {
                        "type": "mapper_parsing_exception",
                        "reason": "failed to parse",
                        "caused_by": {
                            "type": "invalid_shape_exception",
                            "reason": "Bad X value -7703653.0 is not in boundary Rect(minX=-180.0,maxX=180.0,minY=-90.0,maxY=90.0)"
                        }
                    }
                }
                }
            ]
        })
    }

    #[test]
    fn test_elasticsearch_bulk_response_model() {
        let response: ElasticsearchBulkResponse = serde_json::from_value(sample()).unwrap();

        assert_eq!(
            response,
            ElasticsearchBulkResponse {
                items: vec![
                    ElasticsearchBulkItem::Index(ElasticsearchBulkStatus {
                        status: 404,
                        id: "5".to_string(),
                        result: Err(ElasticsearchBulkError {
                            err_type: "document_missing_exception".to_string(),
                            reason: "[_doc][5]: document missing".to_string(),
                            index: "index1".to_string().into(),
                            index_uuid: "aAsFqTI0Tc2W0LCWgPNrOA".to_string().into(),
                            shard: "0".to_string().into(),
                            caused_by: None
                        })
                    }),
                    ElasticsearchBulkItem::Update(ElasticsearchBulkStatus {
                        status: 201,
                        id: "7".to_string(),
                        result: Ok(ElasticsearchBulkResult::Created)
                    })
                ]
            }
        )
    }

    #[test]
    fn test_elasticsearch_bulk_400_error() {
        let response: ElasticsearchBulkResponse =
            serde_json::from_value(sample_400_error()).unwrap();
        assert_eq!(
            response,
            ElasticsearchBulkResponse {
                items: vec![
                    ElasticsearchBulkItem::Index(ElasticsearchBulkStatus {
                        status: 400,
                        id: "StopArea:TCL:01".to_string(),
                        result: Err(ElasticsearchBulkError {
                            err_type: "mapper_parsing_exception".to_string(),
                            reason: "failed to parse".to_string(),
                            index: None,
                            index_uuid: None,
                            shard: None,
                            caused_by: Some(ElasticsearchBulkErrorCausedBy {
                                caused_by_type: "invalid_shape_exception".to_string(),
                                reason: "Bad X value -7703653.0 is not in boundary Rect(minX=-180.0,maxX=180.0,minY=-90.0,maxY=90.0)".to_string()
                            })
                        })
                    })
                ]
            }
        )
    }
}
