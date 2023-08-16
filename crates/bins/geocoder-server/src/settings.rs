use crate::handlers::Settings;
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Arg Match Error: {}", msg))]
    ArgMatch { msg: String },
    #[snafu(display("Arg Missing Error: {}", msg))]
    ArgMissing { msg: String },
    #[snafu(display("Env Var Missing Error: {} [{}]", msg, source))]
    EnvVarMissing { msg: String, source: env::VarError },
    #[snafu(display("Config Merge Error: {} [{}]", msg, source))]
    ConfigMerge {
        msg: String,
        source: config::ConfigError,
    },
    #[snafu(display("Config Value Error: {} [{}]", msg, source))]
    ConfigValue {
        msg: String,
        source: std::num::TryFromIntError,
    },
    #[snafu(display("Config Value Error: {} [{}]", msg, source))]
    ConfigParse {
        msg: String,
        source: std::num::ParseIntError,
    },

    #[snafu(display("Config Compilation Error: {}", source))]
    ConfigCompilation { source: exporter_config::Error },
}

#[derive(Debug, clap::Parser)]
#[command(
    name = "bragi",
    about = "REST API for querying Elasticsearch",
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

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, clap::Parser)]
pub enum Command {
    /// Execute osm2mimir with the given configuration
    Run,
    /// Prints osm2mimir's configuration
    Config,
}

pub fn build_settings(opts: &Opts) -> Result<Settings, Error> {
    exporter_config::config_from(
        opts.config_dir.as_ref(),
        &["bragi", "elasticsearch", "query"],
        opts.run_mode.as_deref(),
        "BRAGI",
        opts.settings.clone(),
    )
    .context(ConfigCompilationSnafu)?
    .try_into()
    .context(ConfigMergeSnafu {
        msg: "cannot merge bragi settings",
    })
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
            run_mode: Some(String::from("testing")),
            settings: vec![],
            cmd: Command::Run,
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().mode, String::from("testing"));
    }

    #[test]
    fn should_override_elasticsearch_port_with_command_line() {
        let config_dir = PathBuf::from(CONFIG_PATH);
        let opts = Opts {
            config_dir,
            run_mode: Some(String::from("testing")),
            settings: vec![String::from("elasticsearch.port=9999")],
            cmd: Command::Run,
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().elasticsearch.url.port().unwrap(), 9999);
    }

    #[test]
    fn should_override_elasticsearch_port_environment_variable() {
        let config_dir = PathBuf::from(CONFIG_PATH);
        println!("{:?}", config_dir);
        std::env::set_var("BRAGI_ELASTICSEARCH__URL", "http://localhost:9999");
        let opts = Opts {
            config_dir,
            run_mode: Some(String::from("testing")),
            settings: vec![],
            cmd: Command::Run,
        };
        let settings = build_settings(&opts);
        assert!(
            settings.is_ok(),
            "Expected Ok, Got an Err: {}",
            settings.unwrap_err()
        );
        assert_eq!(settings.unwrap().elasticsearch.url.port().unwrap(), 9999);
    }
}
