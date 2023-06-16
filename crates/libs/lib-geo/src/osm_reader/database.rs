use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub file: PathBuf,
    pub cache_size: u32,
}
