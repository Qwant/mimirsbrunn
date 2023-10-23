/// This module contains the definition for bano2mimir configuration and command line arguments.
use elastic_client::model::configuration::ContainerConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

use elastic_client::settings::ElasticsearchStorageConfig;
use exporter_config::MimirConfig;
use lib_geo::settings::admin_settings::AdminFromCosmogonyFile;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub id_precision: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAddressesSettings {
    pub mode: Option<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: ContainerConfig,
    pub coordinates: Coordinates,
    #[cfg(feature = "db-storage")]
    pub database: Option<Database>,
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
    /// Override settings values using key=value
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,

    /// Either a single OpenAddresses file, or a directory of several OpenAddresses files.
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
}

impl MimirConfig<'_> for OpenAddressesSettings {
    const ENV_PREFIX: &'static str = "MIMIR";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml", "openaddresses-importer.toml"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let opts = Opts {
            settings: vec![],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = OpenAddressesSettings::get(&opts.settings);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().mode, None);
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let opts = Opts {
            settings: vec!["elasticsearch.url='http://localhost:9999'".to_string()],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = OpenAddressesSettings::get(&opts.settings);
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
        let opts = Opts {
            settings: vec![],
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = OpenAddressesSettings::get(&opts.settings);
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
