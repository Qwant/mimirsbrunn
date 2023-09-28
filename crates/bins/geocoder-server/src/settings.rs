use elastic_client::ElasticsearchStorageConfig;
use elastic_query_builder::settings::QuerySettings;
use serde::{Deserialize, Serialize};
use serde_helpers::deserialize_duration;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, clap::Parser)]
#[command(
name = "bragi",
about = "REST API for querying Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'osm2mimir' subdirectories.
    #[arg(short = 'c', long = "config-dir")]
    pub config_dir: PathBuf,

    /// Defines the run mode in {testing, dev, prod, ...}
    ///
    /// If no run mode is provided, a default behavior will be used.
    #[arg(short = 'm', long = "run-mode")]
    pub run_mode: Option<String>,

    /// Override settings values using key=value
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,
}

pub fn build_settings(opts: &Opts) -> anyhow::Result<Settings> {
    let result = exporter_config::config_from(
        opts.config_dir.as_ref(),
        &["bragi", "elasticsearch", "query"],
        opts.run_mode.as_deref(),
        "BRAGI",
        opts.settings.clone(),
    )?;

    Ok(result.try_into()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exporter_config::CONFIG_PATH;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let config_dir = PathBuf::from(CONFIG_PATH);
        let opts = Opts {
            config_dir,
            run_mode: Some(String::from("testing")),
            settings: vec![],
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().mode, String::from("testing"));
    }

    #[test]
    fn should_override_elasticsearch_port_with_command_line() {
        let config_dir = PathBuf::from(CONFIG_PATH);
        let opts = Opts {
            config_dir,
            run_mode: Some(String::from("testing")),
            settings: vec![String::from("elasticsearch.port=9999")],
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().elasticsearch.url.port().unwrap(), 9999);
    }

    #[test]
    fn should_override_elasticsearch_port_environment_variable() {
        let config_dir = PathBuf::from(CONFIG_PATH);
        println!("{:?}", config_dir);
        std::env::set_var("BRAGI_ELASTICSEARCH__URL", "http://localhost:9999");
        let opts = Opts {
            config_dir,
            run_mode: Some(String::from("testing")),
            settings: vec![],
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().elasticsearch.url.port().unwrap(), 9999);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: String,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub query: QuerySettings,
    pub service: Service,
    pub nb_threads: Option<usize>,
    pub http_cache_duration: usize,
    #[serde(deserialize_with = "deserialize_duration")]
    pub autocomplete_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub reverse_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub features_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Host on which we expose bragi. Example: 'http://localhost', '0.0.0.0'
    pub host: String,
    /// Port on which we expose bragi.
    pub port: u16,
    /// Used on POST request to set an upper limit on the size of the body (in bytes)
    pub content_length_limit: u64,
}
