use aide::OperationIo;
use axum::response::IntoResponse;
use elastic_client::internal::Error;
use elastic_client::remote::RemoteError;
use http::StatusCode;
use schemars::JsonSchema;
use serde::Serialize;

use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;
/// A default error response for most API errors.
#[derive(Debug, Serialize, JsonSchema, OperationIo)]
pub struct AppError {
    /// An error message.
    pub error: String,
    /// A unique error ID.
    pub error_id: Uuid,
    #[serde(skip)]
    pub status: StatusCode,
    /// Optional Additional error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<Value>,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Could not establish Elasticsearch Connection: {}", source)]
    ElasticsearchConnection { source: RemoteError },

    #[error("Could not generate settings: {source}")]
    SettingsProcessing { source: Error },

    #[error("Socket Addr Error with host {host} / port {port}: {source}")]
    SockAddr {
        host: String,
        port: u16,
        source: std::io::Error,
    },

    #[error("Addr Resolution Error {msg}")]
    AddrResolution { msg: String },

    #[error("Could not init logger: {source}")]
    InitLog { source: Error },
}

impl AppError {
    pub fn new(error: &str) -> Self {
        Self {
            error: error.to_string(),
            error_id: Uuid::new_v4(),
            status: StatusCode::BAD_REQUEST,
            error_details: None,
        }
    }

    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.error_details = Some(details);
        self
    }
}

impl From<elastic_client::internal::Error> for AppError {
    fn from(value: Error) -> Self {
        Self {
            error: value.to_string(),
            error_id: Default::default(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_details: None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status;
        let mut res = axum::Json(self).into_response();
        *res.status_mut() = status;
        res
    }
}
