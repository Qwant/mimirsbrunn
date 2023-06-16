use super::model::error::Error as ModelError;
use super::model::status::{Status as DomainStatus, StorageStatus};
use super::ElasticsearchStorage;

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl ElasticsearchStorage {
    pub async fn status(&self) -> Result<DomainStatus, ModelError> {
        let cluster_health = self
            .cluster_health()
            .await
            .map_err(|err| ModelError::Status { source: err.into() })?;
        let cluster_version = self
            .cluster_version()
            .await
            .map_err(|err| ModelError::Status { source: err.into() })?;

        Ok(DomainStatus {
            version: VERSION.to_string(),
            storage: StorageStatus {
                health: cluster_health,
                version: cluster_version,
            },
        })
    }
}
