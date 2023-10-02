use elasticsearch::http::response::Exception;
use semver::Version;
use serde_json::Value;
use thiserror::Error;

use crate::dto::ElasticsearchBulkError;

pub type Result<T> = std::result::Result<T, ElasticClientError>;

#[derive(Debug, Error)]
pub enum ElasticClientError {
    #[error("Elasticsearch error: {0}")]
    ElasticSearchError(#[from] elasticsearch::Error),

    #[error("serde_json error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Config merge error: {0}")]
    ConfigMerge(#[from] config::ConfigError),

    #[error("IO Error: {0}")]
    InvalidIO(#[from] std::io::Error),

    #[error("Invalid json format: {msg} {json}")]
    InvalidJson { msg: String, json: Value },

    #[error("Failed to create elasticsearch index '{0}'")]
    IndexCreationFailed(String),

    #[error("Failed to delete elasticsearch index '{0}'")]
    IndexDeletionFailed(String),

    #[error("Failed to update elasticsearch alias '{0}'")]
    AliasUpdateFailed(String),

    #[error("Failed to update elasticsearch alias '{0}'")]
    InvalidIndexName(String),

    #[error("Object id {object_id}, error: {inner}")]
    BulkObjectCreationFailed {
        object_id: String,
        inner: ElasticsearchBulkError,
    },

    #[error("Failed to create elasticsearch pipeline '{0}'")]
    PipelineCreationFailed(String),

    #[error("Failed to force merge {shard_failed}/{shard_total} shards for {indices}")]
    ForceMergeFailed {
        shard_total: u32,
        shard_failed: u32,
        indices: String,
    },

    #[error("Failed to create elasticsearch template '{0}'")]
    TemplateCreationFailed(String),

    #[error("Elasticsearch health status unknown '{0}'")]
    UnknownElasticSearchStatus(String),

    #[error("Elasticsearch index not found '{0}'")]
    IndexNotFound(String),

    #[error("Elasticsearch exception: status: {status:?}, error: {error:?}")]
    ElasticSearchHttpError {
        error: elasticsearch::http::response::Error,
        status: Option<u16>,
    },

    #[error("No response from elastic search despite the lack of exception")]
    ElasticsearchFailureWithoutException,

    #[error("Unknown configuration directive: '{0}'")]
    InvalidDirective(String),

    #[error("PIT missing from elasticsearch response")]
    ElasticsearchResponseMissingPIT,

    #[error("QueryString not handled for get document by id")]
    QueryStringNotSupported,

    #[error("Elasticsearch version {0}, is not supported")]
    UnsupportedElasticSearchVersion(Version),

    #[error("Semver parse error: {0}")]
    SemVerError(#[from] semver::Error),

    #[error("Elasticsearch client builder error: {0}")]
    ElasticClientBuilderError(#[from] elasticsearch::http::transport::BuildError),

    #[error("Not enough document to publish index, expected {expected}, got {count}")]
    NotEnoughDocument { count: usize, expected: usize },
}

impl From<Exception> for ElasticClientError {
    fn from(exception: Exception) -> Self {
        Self::ElasticSearchHttpError {
            error: exception.error().clone(),
            status: exception.status(),
        }
    }
}
