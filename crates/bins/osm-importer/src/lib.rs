use elastic_client::model::configuration::ContainerConfig;
/// This module contains the definition for osm2mimir configuration and command line arguments.
use elastic_client::settings::ElasticsearchStorageConfig;
use exporter_config::MimirConfig;
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
pub struct OsmSettings {
    pub elasticsearch: ElasticsearchStorageConfig,
    pub pois: Poi,
    pub streets: Street,
    #[serde(rename = "container-poi")]
    pub container_poi: ContainerConfig,
    #[serde(rename = "container-street")]
    pub container_street: ContainerConfig,
    #[cfg(feature = "db-storage")]
    pub database: Option<Database>,
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
    /// Override settings values using key=value
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,

    /// OSM PBF file
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,
}

impl MimirConfig<'_> for OsmSettings {
    const ENV_PREFIX: &'static str = "MIMIR";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml", "osm-importer.toml"]
    }
}
