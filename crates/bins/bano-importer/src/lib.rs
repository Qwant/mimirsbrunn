/// This module contains the definition for bano2mimir configuration and command line arguments.
use elastic_client::model::configuration::ContainerConfig;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::PathBuf;

use elastic_client::ElasticsearchStorageConfig;
use lib_geo::settings::admin_settings::AdminFromCosmogonyFile;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("Config Compilation Error: {}", source))]
    ConfigCompilation { source: exporter_config::Error },
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
#[clap(
name = "bano2mimir",
about = "Parsing BANO document and indexing its content in Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'bano2mimir' subdirectories.
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

    /// Either a single BANO file, or a directory of several BANO files.
    #[clap(short = 'i', long = "input", parse(from_os_str))]
    pub input: PathBuf,

    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, clap::Parser)]
pub enum Command {
    /// Execute bano2mimir with the given configuration
    Run,
    /// Prints bano2mimir's configuration
    Config,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/bano2mimir and <config-dir>/elasticsearch
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
            &["bano2mimir", "elasticsearch"],
            opts.run_mode.as_deref(),
            prefix,
            opts.settings.clone(),
        )
        .context(ConfigCompilationSnafu)?
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
        let config_dir = PathBuf::from(CONFIG_PATH);
        let opts = Opts {
            config_dir,
            run_mode: None,
            settings: vec![String::from("elasticsearch.url='http://localhost:9999'")],
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
