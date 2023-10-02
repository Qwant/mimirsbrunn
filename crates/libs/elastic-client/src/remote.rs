use async_trait::async_trait;
use elasticsearch::http::headers::HeaderMap;
use elasticsearch::http::transport::{SingleNodeConnectionPool, TransportBuilder};
use elasticsearch::http::Method;
use elasticsearch::Elasticsearch;
use semver::{Version, VersionReq};
use serde_json::Value;
use url::Url;

use crate::errors::{ElasticClientError, Result};

use super::{ElasticsearchStorage, ElasticsearchStorageConfig};

#[async_trait]
impl Remote for SingleNodeConnectionPool {
    type Conn = ElasticsearchStorage;
    type Config = ElasticsearchStorageConfig;

    async fn conn(self, config: Self::Config) -> Result<Self::Conn> {
        let version_req = VersionReq::parse(&config.version_req)?;
        let transport = TransportBuilder::new(self).build()?;

        let response = transport
            .send::<String, String>(
                Method::Get,
                "/",
                HeaderMap::new(),
                None, /* query_string */
                None, /* body */
                Some(config.timeout),
            )
            .await?;

        if response.status_code().is_success() {
            let json = response.json::<Value>().await?;
            let version_number = json
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected JSON object".to_string(),
                    json: json.clone(),
                })?
                .get("version")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected 'version'".to_string(),
                    json: json.clone(),
                })?
                .as_object()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: "expected JSON object".to_string(),
                    json: json.clone(),
                })?
                .get("number")
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected 'version.number'"),
                    json: json.clone(),
                })?
                .as_str()
                .ok_or(ElasticClientError::InvalidJson {
                    msg: String::from("expected JSON string"),
                    json: json.clone(),
                })?;
            let version = Version::parse(version_number).unwrap();
            if !version_req.matches(&version) {
                Err(ElasticClientError::UnsupportedElasticSearchVersion(version))
            } else {
                let client = Elasticsearch::new(transport);
                Ok(ElasticsearchStorage { client, config })
            }
        } else {
            Err(ElasticClientError::ElasticsearchFailureWithoutException)
        }
    }
}

/// Opens a connection to elasticsearch given a url
pub fn connection_pool_url(url: &Url) -> SingleNodeConnectionPool {
    SingleNodeConnectionPool::new(url.clone())
}

/// Open a connection to a test elasticsearch
pub fn connection_test_pool() -> SingleNodeConnectionPool {
    let config = ElasticsearchStorageConfig::default_testing();
    connection_pool_url(&config.url)
}

#[async_trait]
pub trait Remote {
    type Conn;
    type Config;

    async fn conn(self, config: Self::Config) -> Result<Self::Conn>;
}
