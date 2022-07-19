/// This module contains the definition for bano2mimir configuration and command line arguments.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdminSettings {
    // will fetch admins from elasticsearch
    Elasticsearch,
    // will fetch admins from a local cosmogony file
    Local(AdminFromCosmogonyFile),
}

impl Default for AdminSettings {
    fn default() -> Self {
        AdminSettings::Elasticsearch
    }
}

impl AdminSettings {
    pub fn build(opt: &Option<AdminFromCosmogonyFile>) -> Self {
        match opt {
            None => AdminSettings::Elasticsearch,
            Some(config) => AdminSettings::Local(config.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminFromCosmogonyFile {
    pub cosmogony_file: PathBuf,

    #[serde(default = "default_french_id_retrocompatibility")]
    pub french_id_retrocompatibility: bool,

    #[serde(default = "default_langs")]
    pub langs: Vec<String>,
}

pub fn default_french_id_retrocompatibility() -> bool {
    true
}

pub fn default_langs() -> Vec<String> {
    vec!["fr".to_string()]
}
