use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaggerError {
    #[cfg(feature = "postal")]
    #[error("LibPostal error: {0}")]
    LibPostalError(#[from] postal::PostalError),
}
