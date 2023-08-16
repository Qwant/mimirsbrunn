/// This module contains the definition for bano2mimir configuration and command line arguments.
use elastic_client::model::configuration::ContainerConfig;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::PathBuf;

use elastic_client::ElasticsearchStorageConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("Config Source Error: {}", source))]
    ConfigSource { source: exporter_config::Error },
    #[snafu(display("Config Error: {}", source))]
    ConfigBuild { source: config::ConfigError },
    #[snafu(display("Invalid Configuration: {}", msg))]
    Invalid { msg: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub langs: Vec<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: ContainerConfig,
    pub nb_threads: Option<usize>,
    pub french_id_retrocompatibility: bool,
    #[serde(default)]
    pub update_templates: bool,
}

#[derive(Debug, clap::Parser)]
#[command(
name = "cosmogony2mimir",
about = "Parsing Cosmogony document and indexing its content in Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'cosmogony2mimir' subdirectories.
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

    /// A file produced by cosmogony
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, clap::Parser)]
pub enum Command {
    /// Execute cosmogony2mimir with the given configuration
    Run,
    /// Prints cosmogony2mimir's configuration
    Config,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/cosmogony2mimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> Result<Self, ConfigError> {
        let prefix = {
            if opts.run_mode.as_deref() == Some("testing") {
                "MIMIR_TEST"
            } else {
                "MIMIR"
            }
        };

        exporter_config::config_from(
            opts.config_dir.as_ref(),
            &["cosmogony2mimir", "elasticsearch"],
            opts.run_mode.as_deref(),
            prefix,
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
    use exporter_config::CONFIG_PATH;

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let config_dir = PathBuf::from(CONFIG_PATH);
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
        assert_eq!(settings.unwrap().mode, None);
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let config_dir = PathBuf::from(CONFIG_PATH);
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
        let config_dir = PathBuf::from(CONFIG_PATH);
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
