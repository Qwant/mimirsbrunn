/// This module contains the definition for bano2mimir configuration and command line arguments.
use elastic_client::model::configuration::ContainerConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

use elastic_client::ElasticsearchStorageConfig;
use lib_geo::settings::admin_settings::AdminFromCosmogonyFile;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub id_precision: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: ContainerConfig,
    pub coordinates: Coordinates,
    #[cfg(feature = "db-storage")]
    pub database: Option<Database>,
    pub nb_threads: Option<usize>,
    #[serde(default)]
    pub update_templates: bool,
    // will read admins from the file if Some(file)
    // will fetch admins from Elasticsearch if None
    pub admins: Option<AdminFromCosmogonyFile>,
}

#[derive(Debug, clap::Parser)]
#[command(
name = "openaddresses2mimir",
about = "Parsing OpenAddresses document and indexing its content in Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'openaddresses2mimir' subdirectories.
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

    /// Either a single OpenAddresses file, or a directory of several OpenAddresses files.
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/openaddresses2mimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> anyhow::Result<Settings> {
        let prefix = {
            if opts.run_mode.as_deref() == Some("testing") {
                "MIMIR_TEST"
            } else {
                "MIMIR"
            }
        };

        let result = exporter_config::config_from(
            opts.config_dir.as_ref(),
            &["openaddresses2mimir", "elasticsearch"],
            opts.run_mode.as_deref(),
            prefix,
            opts.settings.clone(),
        )?;

        result.try_into().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exporter_config::CONFIG_PATH;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let config_dir = CONFIG_PATH.into();
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = Settings::new(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().mode, None);
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let config_dir = CONFIG_PATH.into();
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![String::from("elasticsearch.url='http://localhost:9999'")],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = Settings::new(&opts);
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
        let config_dir = CONFIG_PATH.into();
        std::env::set_var("MIMIR_ELASTICSEARCH__URL", "http://localhost:9999");
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = Settings::new(&opts);
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
