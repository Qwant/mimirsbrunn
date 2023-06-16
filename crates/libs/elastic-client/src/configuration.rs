use config::Config;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("Elasticsearch Index Configuration not found at {}", path.display()))]
    InvalidPath { path: PathBuf },

    #[snafu(display("JSON Serde Serialization Error: {}", source))]
    JsonSerialization {
        source: serde_json::Error,
        details: String,
    },
    #[snafu(display("Invalid Configuration: {} [{}]", source, details))]
    InvalidConfiguration {
        details: String,
        source: config::ConfigError,
    },
}

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
    pub fn new_from_config(config: Config) -> Result<Self, Error> {
        // FIXME Here the error can be misleading. There are two operations, one is getting
        // the key 'elasticsearch', the other is transmogrifying the config into a
        // ComponentTemplateConfiguration.
        config
            .get("elasticsearch")
            .context(InvalidConfigurationSnafu {
                details: String::from("could not get key 'elasticsearch' from configuration"),
            })
    }
    pub fn into_json_body(self) -> Result<serde_json::Value, Error> {
        let name = self.name.clone();
        serde_json::to_value(self).context(JsonSerializationSnafu {
            details: format!("could not serialize component template {}", name),
        })
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
    pub fn new_from_config(config: Config) -> Result<Self, Error> {
        config
            .get("elasticsearch")
            .context(InvalidConfigurationSnafu {
                details: String::from("could not get key 'elasticsearch' from configuration"),
            })
    }
    pub fn into_json_body(self) -> Result<serde_json::Value, Error> {
        let name = self.name.clone();
        serde_json::to_value(self).context(JsonSerializationSnafu {
            details: format!("could not serialize index template {}", name),
        })
    }
}
