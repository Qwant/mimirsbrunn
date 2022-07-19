use mimir::adapters::primary::bragi::handlers::Settings;
use snafu::{ResultExt, Snafu};
use std::{env, path::PathBuf};

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
    ConfigCompilation { source: common::config::Error },
}

#[derive(Debug, clap::Parser)]
#[clap(
    name = "bragi",
    about = "REST API for querying Elasticsearch",
    version = VERSION,
    author = AUTHORS
    )]
pub struct Opts {
    /// Defines the config directory
    ///
    /// This directory must contain 'elasticsearch' and 'osm2mimir' subdirectories.
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

    #[clap(subcommand)]
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
    common::config::config_from(
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

    #[test]
    fn should_return_ok_with_default_config_dir() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
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
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
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
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
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
