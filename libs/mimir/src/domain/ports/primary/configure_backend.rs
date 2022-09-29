use crate::domain::{model::error::Error as ModelError, ports::secondary::storage::Storage};
use async_trait::async_trait;
use config::Config;

#[async_trait(?Send)]
pub trait ConfigureBackend {
    async fn configure(&self, directive: String, config: Config) -> Result<(), ModelError>;
}

#[async_trait(?Send)]
impl<T> ConfigureBackend for T
where
    T: Storage<'static>,
{
    async fn configure(&self, directive: String, config: Config) -> Result<(), ModelError> {
        self.configure(directive, config)
            .await
            .map_err(|err| ModelError::BackendConfiguration { source: err.into() })
    }
}
