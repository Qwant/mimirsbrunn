/// This module contains the definition for ctlmimir configuration and command line arguments.
use config::Config;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::PathBuf;
use structopt::StructOpt;

use mimir::adapters::secondary::elasticsearch::ElasticsearchStorageConfig;

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
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub logging: Logging,
    pub elasticsearch: ElasticsearchStorageConfig,
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "ctlmimir",
    about = "Configure Elasticsearch Backend",
    version = VERSION,
    author = AUTHORS
    )]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'ctlmimir' subdirectories.
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

    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Execute ctlmimir with the given configuration
    Run,
    /// Prints ctlmimir's configuration
    Config,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/ctlmimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> Result<Self, Error> {
        let mut config = Config::default();

        config
            .set_default("path", opts.config_dir.display().to_string())
            .context(ConfigMerge { msg: "path" })?;

        config
            .with_merged(
                common::config::config_from(
                    opts.config_dir.as_ref(),
                    &["elasticsearch", "logging"],
                    opts.run_mode.as_deref(),
                    "CTLMIMIR",
                    opts.settings.clone(),
                )
                .context(ConfigCompilation)?,
            )
            .context(ConfigMerge {
                msg: "Cannot build the configuration from sources",
            })?
            .try_into()
            .context(ConfigMerge {
                msg: "Cannot convert configuration into ctlmimir settings",
            })
    }
}
