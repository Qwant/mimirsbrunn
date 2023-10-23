use std::time::Duration;

use exporter_config::MimirConfig;
use serde::{Deserialize, Serialize};
use url::Url;

use serde_helpers::deserialize_duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ElasticsearchStorageConfig {
    pub url: Url,
    /// Prefix used for all indexes that mimir interacts with
    pub index_root: String,
    /// Timeout in milliseconds on client calls to Elasticsearch.
    #[serde(deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
    /// Constraint on the version of Elasticsearch.
    pub version_req: String,
    /// Number of documents loaded per request when performing a `list_documents`
    pub scroll_chunk_size: u64,
    /// Liveness of the PIT while performing a `list_documents`.
    pub scroll_pit_alive: String,
    /// Max of concurrent requests during insertion.
    pub insertion_concurrent_requests: usize,
    /// Number of document per request during insertion.
    pub insertion_chunk_size: usize,
    /// Number of shards copies that must be active before performing indexing
    ///  operations.
    pub wait_for_active_shards: u64,
    pub force_merge: ElasticsearchStorageForceMergeConfig,
    /// Setup a backoff to wait after a bulk operation fail and retry the operation,
    /// each successive retry will wait twice as long as the previous one.
    pub bulk_backoff: ElasticsearchStorageBackoffConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ElasticsearchStorageForceMergeConfig {
    ///  Force refresh before force_merge
    pub refresh: bool,
    /// If this is set to `true` a force merge will be performed after an index
    /// is published. For more details see
    /// https://www.elastic.co/guide/en/elasticsearch/reference/7.17/indices-forcemerge.html
    pub enabled: bool,
    /// Number of segments to merge to, uses ES's default behavior by default
    pub max_number_segments: Option<i64>,
    /// Timeout in milliseconds for the forcemerge operation
    #[serde(deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
    /// Allow the forcemerge query to timeout, which would only result in a
    /// warning. Note that the forcemerge operation will still continue to be
    /// performed in the background anyway.
    pub allow_timeout: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ElasticsearchStorageBackoffConfig {
    /// Number of retries after the first failure (set 0 to never retry)
    pub retry: u8,
    /// Waiting time in milliseconds after the first failure
    #[serde(deserialize_with = "deserialize_duration")]
    pub wait: Duration,
}

impl MimirConfig<'_> for ElasticsearchStorageConfig {
    const ENV_PREFIX: &'static str = "ELASTICSEARCH";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml", "osm-importer.toml"]
    }

    fn root_key() -> Option<&'static str> {
        Some("elasticsearch")
    }
}

#[cfg(test)]
mod test {
    use crate::settings::ElasticsearchStorageConfig;
    use exporter_config::MimirConfig;

    #[test]
    fn test() {
        let url = "url='http://localhost:9999'".to_string();
        assert!(ElasticsearchStorageConfig::get(&[url]).is_ok());
    }
}
