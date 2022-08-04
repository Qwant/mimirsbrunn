use async_trait::async_trait;
use snafu::Snafu;

use crate::domain::model::{error::Error as ModelError, status::StorageStatus};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Health Retrieval Error: {}", source))]
    HealthRetrievalError { source: Box<dyn std::error::Error> },
    #[snafu(display("Version Retrieval Error: {}", source))]
    VersionRetrievalError { source: Box<dyn std::error::Error> },
}

#[async_trait]
pub trait Status {
    async fn status(&self) -> Result<StorageStatus, Error>;
}

#[async_trait]
impl<T: ?Sized> Status for Box<T>
where
    T: Status + Send + Sync,
{
    async fn status(&self) -> Result<StorageStatus, Error> {
        (**self).status().await
    }
}

// Conversion from secondary ports errors
impl From<Error> for ModelError {
    fn from(err: Error) -> ModelError {
        match err {
            Error::HealthRetrievalError { source } => ModelError::DocumentRetrievalError { source },
            Error::VersionRetrievalError { source } => {
                ModelError::DocumentRetrievalError { source }
            }
        }
    }
}
