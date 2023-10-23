/// This module contains the definition for bano2mimir configuration and command line arguments.
use elastic_client::model::configuration::ContainerConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

use elastic_client::settings::ElasticsearchStorageConfig;
use exporter_config::MimirConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosmogonySettings {
    pub mode: Option<String>,
    pub langs: Vec<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: ContainerConfig,
    pub french_id_retrocompatibility: bool,
    #[serde(default)]
    pub update_templates: bool,
}

impl MimirConfig<'_> for CosmogonySettings {
    const ENV_PREFIX: &'static str = "MIMIR";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml", "cosmogony-importer.toml"]
    }
}

#[derive(Debug, clap::Parser)]
#[command(
name = "cosmogony2mimir",
about = "Parsing Cosmogony document and indexing its content in Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Override settings values using key=value
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,

    /// A file produced by cosmogony
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let opts = Opts {
            settings: vec![],
            input: PathBuf::from("foo.jsonl.gz"),
        };
        let settings = CosmogonySettings::get(&opts.settings);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().mode, None);
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let overrides = &["elasticsearch.url='http://localhost:9999'".to_string()];
        let settings = CosmogonySettings::get(overrides);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(
            settings.unwrap().elasticsearch.url.as_str(),
            "http://localhost:9999/"
        );
    }

    #[test]
    fn should_override_elasticsearch_url_environment_variable() {
        std::env::set_var("MIMIR__elasticsearch__url", "http://localhost:9999");
        let settings = CosmogonySettings::get(&[]);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(
            settings.unwrap().elasticsearch.url.as_str(),
            "http://localhost:9999/"
        );
    }
}
