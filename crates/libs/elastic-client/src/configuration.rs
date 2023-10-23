use crate::errors::Result;
use config::Config;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexSettings(serde_json::Value);

impl std::fmt::Display for IndexSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}

impl IndexSettings {
    pub fn new(value: serde_json::Value) -> IndexSettings {
        IndexSettings(value)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexMappings(serde_json::Value);

impl std::fmt::Display for IndexMappings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}

impl IndexMappings {
    pub fn new(value: serde_json::Value) -> IndexMappings {
        IndexMappings(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mappings: Option<IndexMappings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<IndexSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTemplateConfiguration {
    #[serde(skip_serializing)]
    pub name: String,
    pub template: Template,
}

impl ComponentTemplateConfiguration {
    pub fn new_from_config(config: Config) -> Result<Self> {
        let elasticsearch_config = config.get("elasticsearch")?;
        Ok(elasticsearch_config)
    }

    pub fn into_json_body(self) -> Result<serde_json::Value> {
        let value = serde_json::to_value(self)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexTemplateConfiguration {
    #[serde(skip_serializing)]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<Template>,
    pub composed_of: Vec<String>,
    pub index_patterns: Vec<String>,
    pub version: u32,
    pub priority: u32,
}

impl IndexTemplateConfiguration {
    pub fn new_from_config(config: Config) -> Result<Self> {
        let elasticsearch_config = config.get("elasticsearch")?;
        Ok(elasticsearch_config)
    }

    pub fn into_json_body(self) -> Result<serde_json::Value> {
        let value = serde_json::to_value(self)?;
        Ok(value)
    }
}
