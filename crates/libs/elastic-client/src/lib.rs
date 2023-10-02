use elasticsearch::Elasticsearch;
use exporter_config::CONFIG_PATH;
use serde::{Deserialize, Serialize};
use serde_helpers::deserialize_duration;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

pub mod configuration;
pub mod dto;
pub mod future_helper;
pub mod generate_index;
pub mod internal;
pub mod model;
pub mod remote;
pub mod status;
pub mod storage;
pub mod templates;

pub mod errors;

/// A structure wrapping around the elasticsearch's client.
#[derive(Clone, Debug)]
pub struct ElasticsearchStorage {
    /// Elasticsearch client
    pub client: Elasticsearch,
    /// Client configuration
    pub config: ElasticsearchStorageConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ElasticsearchStorageConfig {
    pub url: Url,
    pub index_root: String,
    #[serde(deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
    pub version_req: String,
    pub scroll_chunk_size: u64,
    pub scroll_pit_alive: String,
    pub insertion_concurrent_requests: usize,
    pub insertion_chunk_size: usize,
    pub wait_for_active_shards: u64,
    pub force_merge: ElasticsearchStorageForceMergeConfig,
    pub bulk_backoff: ElasticsearchStorageBackoffConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ElasticsearchStorageBackoffConfig {
    retry: u8,
    #[serde(deserialize_with = "deserialize_duration")]
    wait: Duration,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ElasticsearchStorageForceMergeConfig {
    pub refresh: bool,
    pub enabled: bool,
    pub max_number_segments: Option<i64>,
    #[serde(deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
    pub allow_timeout: bool,
}

impl Default for ElasticsearchStorageConfig {
    /// We retrieve the elasticsearch configuration from ./config/elasticsearch/default.
    fn default() -> Self {
        let config = exporter_config::config_from(
            &PathBuf::from(CONFIG_PATH),
            &["elasticsearch"],
            None,
            None,
            vec![],
        );

        config
            .expect("cannot build the configuration for testing from config")
            .get("elasticsearch")
            .expect("expected elasticsearch section in configuration from config")
    }
}

impl ElasticsearchStorageConfig {
    pub fn default_testing() -> Self {
        let config_dir = PathBuf::from(CONFIG_PATH);
        let config = exporter_config::config_from(
            config_dir.as_path(),
            &["elasticsearch"],
            "testing",
            "MIMIR_TEST",
            vec![],
        );

        config
            .unwrap_or_else(|_| {
                panic!(
                    "cannot build the configuration for testing from {}",
                    config_dir.display(),
                )
            })
            .get("elasticsearch")
            .unwrap_or_else(|_| {
                panic!(
                    "expected elasticsearch section in configuration from {}",
                    config_dir.display(),
                )
            })
    }
}

#[cfg(test)]
pub mod tests {

    use serde::{Deserialize, Serialize};
    use serial_test::serial;

    use super::*;

    use crate::errors::ElasticClientError;
    use crate::remote::Remote;
    use model::configuration::{ContainerConfig, ContainerVisibility};
    use places::{ContainerDocument, Document};

    #[tokio::test]
    #[serial]
    async fn should_connect_to_elasticsearch() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let _client = remote::connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Elasticsearch Connection Established");
    }

    #[tokio::test]
    #[serial]
    async fn should_create_index() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let client = remote::connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Elasticsearch Connection Established");

        let config = ContainerConfig {
            name: "foo".to_string(),
            dataset: "bar".to_string(),
            visibility: ContainerVisibility::Public,
            number_of_shards: 1,
            number_of_replicas: 0,
            min_expected_count: 1,
        };

        let res = client.create_container(&config).await;
        assert!(res.is_ok());
    }

    #[derive(Deserialize, Serialize)]
    struct TestObj {
        value: String,
    }

    impl Document for TestObj {
        fn id(&self) -> String {
            self.value.clone()
        }
    }

    impl ContainerDocument for TestObj {
        fn static_doc_type() -> &'static str {
            "test-obj"
        }
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_insert_multiple_documents() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let client = remote::connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Elasticsearch Connection Established");

        let config = ContainerConfig {
            name: TestObj::static_doc_type().to_string(),
            dataset: "default".to_string(),
            visibility: ContainerVisibility::Public,
            number_of_shards: 1,
            number_of_replicas: 0,
            min_expected_count: 1,
        };

        client
            .create_container(&config)
            .await
            .expect("container creation");

        let documents = vec![
            TestObj {
                value: String::from("obj1"),
            },
            TestObj {
                value: String::from("obj2"),
            },
            TestObj {
                value: String::from("obj3"),
            },
            TestObj {
                value: String::from("obj4"),
            },
            TestObj {
                value: String::from("obj5"),
            },
            TestObj {
                value: String::from("obj6"),
            },
        ];
        let documents = futures::stream::iter(documents);

        let res = client
            .insert_documents(
                String::from("root_obj_dataset_test-index-bulk-insert"),
                documents,
            )
            .await;

        assert_eq!(res.expect("insertion stats").created, 6);
    }

    #[tokio::test]
    #[serial]
    async fn should_detect_invalid_elasticsearch_version() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let res = remote::connection_test_pool()
            .conn(ElasticsearchStorageConfig {
                version_req: ">=9.99.99".to_string(),
                ..ElasticsearchStorageConfig::default_testing()
            })
            .await;

        let error = res.unwrap_err();
        assert!(matches!(
            error,
            ElasticClientError::UnsupportedElasticSearchVersion(_)
        ));
    }
}
