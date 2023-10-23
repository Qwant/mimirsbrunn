use elasticsearch::http::headers::HeaderMap;
use elasticsearch::http::transport::{SingleNodeConnectionPool, TransportBuilder};
use elasticsearch::http::Method;
use elasticsearch::Elasticsearch;
use semver::{Version, VersionReq};
use serde_json::Value;

use crate::errors::{ElasticClientError, Result};

use super::{ElasticSearchClient, ElasticsearchStorageConfig};

impl ElasticSearchClient {
    pub async fn conn(config: ElasticsearchStorageConfig) -> Result<Self> {
        let pool = SingleNodeConnectionPool::new(config.url.clone());
        let version_req = VersionReq::parse(&config.version_req)?;
        let transport = TransportBuilder::new(pool).build()?;

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
                Ok(ElasticSearchClient { client, config })
            }
        } else {
            Err(ElasticClientError::ElasticsearchFailureWithoutException)
        }
    }
}
