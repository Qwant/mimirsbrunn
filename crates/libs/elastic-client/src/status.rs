use super::model::status::{Status as DomainStatus, StorageStatus};
use super::ElasticsearchStorage;
use crate::errors::Result;

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl ElasticsearchStorage {
    pub async fn status(&self) -> Result<DomainStatus> {
        let cluster_health = self.cluster_health().await?;
        let cluster_version = self.cluster_version().await?;

        Ok(DomainStatus {
            version: VERSION.to_string(),
            storage: StorageStatus {
                health: cluster_health,
                version: cluster_version,
            },
        })
    }
}
