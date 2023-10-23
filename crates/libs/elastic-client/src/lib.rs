use elasticsearch::Elasticsearch;

use crate::settings::ElasticsearchStorageConfig;

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

pub mod settings;

#[derive(Clone, Debug)]
pub struct ElasticSearchClient {
    /// Elasticsearch client
    pub client: Elasticsearch,
    /// Client configuration
    pub config: ElasticsearchStorageConfig,
}

#[cfg(test)]
pub mod tests {
    use crate::errors::ElasticClientError;
    use crate::model::configuration::{ContainerConfig, ContainerVisibility};
    use crate::settings::ElasticsearchStorageConfig;
    use crate::ElasticSearchClient;
    use exporter_config::MimirConfig;
    use places::{ContainerDocument, Document};
    use serde::{Deserialize, Serialize};
    use serial_test::serial;
    use speculoos::prelude::*;
    use test_containers::ElasticSearchContainer;

    const ELASTIC_TEST_URL: &str = "url='http://localhost:9200'";
    #[tokio::test]
    #[serial]
    async fn should_connect_to_elasticsearch() -> anyhow::Result<()> {
        ElasticSearchContainer::start_and_build_client().await?;
        let config = ElasticsearchStorageConfig::get(&[ELASTIC_TEST_URL.to_string()])?;
        let conn = ElasticSearchClient::conn(config).await;

        assert!(conn.is_ok());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_create_index() -> anyhow::Result<()> {
        // Arrange
        ElasticSearchContainer::start_and_build_client().await?;

        let config = ElasticsearchStorageConfig::get(&[ELASTIC_TEST_URL.to_string()])?;

        let conn = ElasticSearchClient::conn(config).await?;

        let config = ContainerConfig {
            name: "foo".to_string(),
            dataset: "bar".to_string(),
            visibility: ContainerVisibility::Public,
            number_of_shards: 1,
            number_of_replicas: 0,
            min_expected_count: 1,
        };

        // Act
        let res = conn.create_container(&config).await;

        // Assert
        assert_that!(res).is_ok();
        Ok(())
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
    async fn should_correctly_insert_multiple_documents() -> anyhow::Result<()> {
        // Arrange
        let config = ElasticsearchStorageConfig::get(&[ELASTIC_TEST_URL.to_string()])?;
        let conn = ElasticSearchClient::conn(config).await?;

        let config = ContainerConfig {
            name: TestObj::static_doc_type().to_string(),
            dataset: "default".to_string(),
            visibility: ContainerVisibility::Public,
            number_of_shards: 1,
            number_of_replicas: 0,
            min_expected_count: 1,
        };

        conn.create_container(&config).await?;

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

        // Act
        let res = conn
            .insert_documents(
                String::from("root_obj_dataset_test-index-bulk-insert"),
                documents,
            )
            .await?;

        // Assert
        assert_that!(res.created).is_equal_to(6);

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_detect_invalid_elasticsearch_version() -> anyhow::Result<()> {
        // Arrange
        let req = "version_req='>=9.99.99'".to_string();
        let config = ElasticsearchStorageConfig::get(&[req, ELASTIC_TEST_URL.to_string()])?;

        // Act
        let conn = ElasticSearchClient::conn(config).await;

        // Assert
        assert_that!(conn)
            .is_err()
            .matches(|err| matches!(err, ElasticClientError::UnsupportedElasticSearchVersion(_)));

        Ok(())
    }
}
