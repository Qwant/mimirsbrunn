use bollard::Docker;
use elastic_client::settings::ElasticsearchStorageConfig;

use elastic_client::ElasticSearchClient;
use elasticsearch::cluster::ClusterDeleteComponentTemplateParts;
use elasticsearch::indices::{
    IndicesDeleteAliasParts, IndicesDeleteIndexTemplateParts, IndicesDeleteParts,
};
use exporter_config::MimirConfig;

use std::time::Duration;

use crate::container::Container;
use crate::wait::ReadyCondition;

mod container;
mod port;
mod wait;

pub struct ElasticSearchContainer {
    inner: Container,
}

impl ElasticSearchContainer {
    pub async fn start_and_build_client() -> anyhow::Result<ElasticSearchClient> {
        let config = ElasticsearchStorageConfig::get(&["timeout=10000".to_string()])?;

        let client = if std::env::var("TEST_CONTAINER") != Ok("false".to_string()) {
            let container = Self {
                inner: Container {
                    image: "docker.elastic.co/elasticsearch/elasticsearch:7.13.0".to_string(),
                    name: "mimir_test_es".to_string(),
                    client: Docker::connect_with_socket_defaults().unwrap(),
                    env: vec![
                        ("xpack.security.enabled".to_string(), "false".to_string()),
                        ("discovery.type".to_string(), "single-node".to_string()),
                    ],
                    ready_condition: ReadyCondition::HttpPull {
                        url: "http://localhost:9200".to_string(),
                        expect: r#""status": "green""#.to_string(),
                        interval: Duration::from_millis(100),
                    },
                    memory: Some(1073741824),
                    memory_swap: Some(1073741824),
                    exposed_port: vec![(9200, 9200)],
                },
            };

            let running = container.inner.is_running().await;

            // If the container is not running remove the previous one
            if !running {
                let _ = container
                    .inner
                    .client
                    .remove_container(&container.inner.name, None)
                    .await;
                container.inner.run().await?;
                ElasticSearchClient::conn(config).await?
            } else {
                let client = ElasticSearchClient::conn(config).await?;
                container.cleanup(client.clone()).await?;
                client
            }
        } else {
            ElasticSearchClient::conn(config).await?
        };

        client.update_templates().await?;
        Ok(client)
    }

    async fn cleanup(&self, client: ElasticSearchClient) -> anyhow::Result<()> {
        let _ = client
            .client
            .indices()
            .delete(IndicesDeleteParts::Index(&["*"]))
            .request_timeout(client.config.timeout)
            .send()
            .await?;

        client
            .client
            .indices()
            .delete_alias(IndicesDeleteAliasParts::IndexName(&["*"], &["*"]))
            .request_timeout(client.config.timeout)
            .send()
            .await?;

        client
            .client
            .indices()
            .delete_index_template(IndicesDeleteIndexTemplateParts::Name("munin_*"))
            .request_timeout(client.config.timeout)
            .send()
            .await?;

        client
            .client
            .cluster()
            .delete_component_template(ClusterDeleteComponentTemplateParts::Name("mimir-*"))
            .request_timeout(client.config.timeout)
            .send()
            .await?;

        Ok(())
    }
}
