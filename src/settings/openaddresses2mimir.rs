/// This module contains the definition for bano2mimir configuration and command line arguments.
use config::Config;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::PathBuf;
use structopt::StructOpt;

use mimir2::adapters::secondary::elasticsearch::ElasticsearchStorageConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Config Compilation Error: {}", source))]
    ConfigCompilation { source: common::config::Error },
    #[snafu(display("Config Merge Error: {} [{}]", msg, source))]
    ConfigMerge {
        msg: String,
        source: config::ConfigError,
    },
    #[snafu(display("Invalid Configuration: {}", msg))]
    Invalid { msg: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub dataset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concurrency {
    pub nb_threads: usize,
    pub nb_insert_threads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    pub id_precision: usize,
}

#[cfg(feature = "db-storage")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub file: PathBuf,
    pub buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub logging: Logging,
    pub concurrency: Concurrency,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub container: Container,
    pub coordinates: Coordinates,
    #[cfg(feature = "db-storage")]
    pub database: Option<Database>,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "openaddresses2mimir",
    about = "Parsing OpenAddresses document and indexing its content in Elasticsearch",
    version = VERSION,
    author = AUTHORS
    )]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'openaddresses2mimir' subdirectories.
    #[structopt(parse(from_os_str), short = "c", long = "config-dir")]
    pub config_dir: PathBuf,

    /// Defines the run mode in {testing, dev, prod, ...}
    ///
    /// If no run mode is provided, a default behavior will be used.
    #[structopt(short = "m", long = "run-mode")]
    pub run_mode: Option<String>,

    /// Override settings values using key=value
    #[structopt(short = "s", long = "setting")]
    pub settings: Vec<String>,

    /// Either a single OpenAddresses file, or a directory of several OpenAddresses files.
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    pub input: PathBuf,

    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Execute openaddresses2mimir with the given configuration
    Run,
    /// Prints openaddresses2mimir's configuration
    Config,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/openaddresses2mimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> Result<Self, Error> {
        let mut builder = Config::builder();

        builder = builder.add_source(
            common::config::config_from(
                opts.config_dir.as_ref(),
                &["openaddresses2mimir", "elasticsearch"],
                opts.run_mode.as_deref(),
                "MIMIR",
                opts.settings.clone(),
            )
            .context(ConfigCompilation)?,
        );

        let config = builder.build().context(ConfigMerge {
            msg: String::from("Cannot build the configuration from sources"),
        })?;

        config.try_into().context(ConfigMerge {
            msg: String::from("Cannot convert configuration into openaddresses2mimir settings"),
        })
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
            input: PathBuf::from("foo.osm.pbf"),
        };
        let settings = Settings::new(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err().to_string()
        );
        assert_eq!(settings.unwrap().mode, None);
    }

    #[test]
    fn should_override_elasticsearch_url_with_command_line() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
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
            settings.unwrap_err().to_string()
        );
        assert_eq!(
            settings.unwrap().elasticsearch.url.as_str(),
            "http://localhost:9999/"
        );
    }

    #[test]
    fn should_override_elasticsearch_url_environment_variable() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        std::env::set_var("MIMIR_ELASTICSEARCH_URL", "http://localhost:9999");
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
            settings.unwrap_err().to_string()
        );
        assert_eq!(
            settings.unwrap().elasticsearch.url.as_str(),
            "http://localhost:9999/"
        );
    }
}