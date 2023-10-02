use elastic_client::errors::ElasticClientError;
use thiserror::Error;
use validator::ValidationErrors;

#[derive(Debug, Error)]
pub enum GeoError {
    #[error("Elasticsearch client error {0}")]
    ElasticsearchPool(#[from] ElasticClientError),

    #[error("Cosmogony error: {0}")]
    Cosmogony(#[from] anyhow::Error),

    #[error("No admins were retrieved from ES")]
    NoImportedAdmins,

    #[error("invalid insee id: `{}`", id)]
    InvalidInseeId { id: String },

    #[error("invalid fantoir id: `{}`", id)]
    InvalidFantoirId { id: String },

    #[error("Invalid coordinates: {0}")]
    InvalidCoordinates(#[from] ValidationErrors),
}
