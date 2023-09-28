use thiserror::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0:?}")]
    Io(std::io::Error),
    /// This was previously `GeoJsonUnknownType`, but has been split for clarity
    #[error("Expected type: `{expected_type}`, but found `{found_type}`")]
    InvalidGeometryConversion {
        expected_type: &'static str,
        found_type: &'static str,
    },
    #[error("Error while deserializing JSON: {0:?}")]
    MalformedJson(serde_json::Error),
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::MalformedJson(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
