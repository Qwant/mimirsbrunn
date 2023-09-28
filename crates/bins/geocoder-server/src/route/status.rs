use crate::errors::AppError;
use crate::extractors::Json;
use crate::AppState;
use aide::transform::TransformOperation;
use axum::extract::State;
use http::StatusCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct BragiStatus {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MimirStatus {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ElasticsearchStatus {
    pub version: String,
    pub health: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StatusResponseBody {
    pub bragi: BragiStatus,
    pub mimir: MimirStatus,
    pub elasticsearch: ElasticsearchStatus,
}

pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponseBody>, AppError> {
    match state.client.status().await {
        Ok(res) => {
            let resp = StatusResponseBody {
                bragi: BragiStatus {
                    version: VERSION.to_string(),
                },
                mimir: MimirStatus {
                    version: res.version,
                },
                elasticsearch: ElasticsearchStatus {
                    version: res.storage.version,
                    health: res.storage.health.to_string(),
                    url: state.settings.elasticsearch.url.to_string(),
                },
            };
            Ok(Json(resp))
        }
        Err(err) => {
            Err(AppError::new(&err.to_string()).with_status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

pub fn status_docs(op: TransformOperation) -> TransformOperation {
    op.description("Status")
        .response::<200, Json<StatusResponseBody>>()
}
