use anyhow::anyhow;
use elastic_client::model::configuration::ContainerConfig;
/// This module contains the definition for osm2mimir configuration and command line arguments.
use elastic_client::ElasticsearchStorageConfig;
use lib_geo::settings::admin_settings::AdminFromCosmogonyFile;
use serde::{Deserialize, Serialize};
use serde_helpers::usize1000;
use std::env;
use std::path::PathBuf;

#[cfg(feature = "db-storage")]
use lib_geo::osm_reader::database::Database;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Street {
    pub import: bool,
    pub exclusions: lib_geo::osm_reader::street::StreetExclusion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poi {
    pub import: bool,
    #[serde(default = "usize1000")]
    pub max_distance_reverse: usize, // in meters
    pub config: Option<lib_geo::osm_reader::poi::PoiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub pois: Poi,
    pub streets: Street,
    #[serde(rename = "container-poi")]
    pub container_poi: ContainerConfig,
    #[serde(rename = "container-street")]
    pub container_street: ContainerConfig,
    #[cfg(feature = "db-storage")]
    pub database: Option<Database>,
    pub nb_threads: Option<usize>,
    #[serde(default)]
    pub update_templates: bool,

    // will read admins from the file if Some(file)
    // will fetch admins from Elasticsearch if None
    pub admins: Option<AdminFromCosmogonyFile>,
}

pub fn default_langs() -> Vec<String> {
    vec!["fr".to_string()]
}

#[derive(Debug, clap::Parser)]
#[command(
name = "osm2mimir",
about = "Parsing OSM PBF document and indexing its content in Elasticsearch",
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

    /// OSM PBF file
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/osm2mimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> anyhow::Result<Settings> {
        let prefix = {
            if opts.run_mode.as_deref() == Some("testing") {
                "MIMIR_TEST"
            } else {
                "MIMIR"
            }
        };

        exporter_config::config_from(
            opts.config_dir.as_ref(),
            &["osm2mimir", "elasticsearch"],
            opts.run_mode.as_deref(),
            prefix,
            opts.settings.clone(),
        )?
        .try_into()
        .map_err(Into::into)
    }
}

// This function returns an error if the settings are invalid.
pub fn validate(settings: Settings) -> anyhow::Result<Settings> {
    let import_streets_enabled = settings.streets.import;

    let import_poi_enabled = settings.pois.import;

    if !import_streets_enabled && !import_poi_enabled {
        return Err(anyhow!("Neither streets nor POIs import is enabled. Nothing to do. Use -s pois.import=true or -s streets.import=true"));
    }
    Ok(settings)
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
    fn should_override_elasticsearch_port_with_command_line() {
        let config_dir = PathBuf::from(CONFIG_PATH);
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
    fn should_override_elasticsearch_port_environment_variable() {
        let config_dir = PathBuf::from(CONFIG_PATH);
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
