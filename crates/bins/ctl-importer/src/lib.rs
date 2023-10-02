/// This module contains the definition for ctlmimir configuration and command line arguments.
use config::Config;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

use elastic_client::ElasticsearchStorageConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub mode: Option<String>,
    pub elasticsearch: ElasticsearchStorageConfig,
    pub nb_threads: Option<usize>,
}

#[derive(Debug, clap::Parser)]
#[command(
name = "ctlmimir",
about = "Configure Elasticsearch Backend",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    /// Defines the run mode in {testing, dev, prod, ...}
    ///
    /// If no run mode is provided, a default behavior will be used.
    #[arg(short = 'm', long = "run-mode")]
    pub run_mode: Option<String>,

    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'ctlmimir' subdirectories.
    #[arg(short = 'c', long = "config-dir")]
    pub config_dir: PathBuf,

    /// Override settings values using key=value
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,
}

// TODO Parameterize the config directory
impl Settings {
    // Read the configuration from <config-dir>/ctlmimir and <config-dir>/elasticsearch
    pub fn new(opts: &Opts) -> anyhow::Result<Self> {
        let mut config = Config::default();

        config.set_default("path", opts.config_dir.display().to_string())?;

        config
            .with_merged(exporter_config::config_from(
                opts.config_dir.as_ref(),
                &["elasticsearch"],
                opts.run_mode.as_deref(),
                "MIMIR",
                opts.settings.clone(),
            )?)?
            .try_into()
            .map_err(Into::into)
    }
}
