use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OsmReaderError {
    #[error("IO Error: {0}")]
    IO(#[from] io::Error),

    #[cfg(feature = "db-storage")]
    #[error("Sqlite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("OsmPbfReader extraction error: {0}")]
    OsmPbfReaderExtraction(#[from] osmpbfreader::Error),

    #[error("Json error: [{0}]")]
    JsonDeserialization(#[from] serde_json::Error),

    #[error("Poi validation error: {0}")]
    PoiValidation(String),

    #[error("Config error: {0}")]
    ConfigMerge(#[from] config::ConfigError),
}
