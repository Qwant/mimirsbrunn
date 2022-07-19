/// This module contains the definition for bano2mimir configuration and command line arguments.
use mimir::domain::model::configuration::{ContainerConfig, PhysicalModeWeight};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::{env, path::PathBuf};

use mimir::adapters::secondary::elasticsearch::ElasticsearchStorageConfig;

use super::admin_settings::AdminFromCosmogonyFile;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Config Source Error: {}", source))]
    ConfigSource { source: common::config::Error },
    #[snafu(display("Config Error: {}", source))]
    ConfigBuild { source: config::ConfigError },
    #[snafu(display("Invalid Configuration: {}", msg))]
    Invalid { msg: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: ContainerConfig,
    pub nb_threads: Option<usize>,
    pub physical_mode_weight: Option<Vec<PhysicalModeWeight>>,
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
#[clap(
    name = "ntfs2mimir",
    about = "Parsing NTFS document and indexing its content in Elasticsearch",
    version = VERSION,
    author = AUTHORS
    )]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'cosmogony2mimir' subdirectories.
    #[clap(parse(from_os_str), short = 'c', long = "config-dir")]
    pub config_dir: PathBuf,

    /// Defines the run mode in {testing, dev, prod, ...}
    ///
    /// If no run mode is provided, a default behavior will be used.
    #[clap(short = 'm', long = "run-mode")]
    pub run_mode: Option<String>,

    /// Override settings values using key=value
    #[clap(
        short = 's',
        long = "setting",
        multiple_values = false,
        multiple_occurrences = true
    )]
    pub settings: Vec<String>,

    /// Either a NTFS zipped file, or the directory in which an NTFS file has been unzipped.
    #[clap(short = 'i', long = "input", parse(from_os_str))]
    pub input: PathBuf,

    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, clap::Parser)]
pub enum Command {
    /// Execute ntfs2mimir with the given configuration
    Run,
    /// Prints ntfs2mimir's configuration
    Config,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/ntfs2mimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> Result<Self, Error> {
        common::config::config_from(
            opts.config_dir.as_ref(),
            &["ntfs2mimir", "elasticsearch"],
            opts.run_mode.as_deref(),
            "MIMIR",
            opts.settings.clone(),
        )
        .context(ConfigSourceSnafu)?
        .try_into()
        .context(ConfigBuildSnafu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![],
            cmd: Command::Run,
            input: PathBuf::from("foo.jsonl.gz"),
        };
        let settings = Settings::new(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        let settings_unwrap = settings.unwrap();
        assert_eq!(settings_unwrap.mode, None);
        assert!(!settings_unwrap.physical_mode_weight.unwrap().is_empty());
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![String::from("elasticsearch.url='http://localhost:9999'")],
            cmd: Command::Run,
            input: PathBuf::from("foo.jsonl.gz"),
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
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        std::env::set_var("MIMIR_ELASTICSEARCH__URL", "http://localhost:9999");
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![],
            cmd: Command::Run,
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
