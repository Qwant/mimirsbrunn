use crate::document::ContainerDocument;
use config::{Config, Environment, File};
use snafu::{ResultExt, Snafu};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Key Value Splitting Error: {}", msg))]
    Splitting { msg: String },

    #[snafu(display("Setting Config Value Error: {}", source))]
    ConfigValue { source: config::ConfigError },

    #[snafu(display("Config Compilation Error: {}", source))]
    ConfigCompilation { source: config::ConfigError },
}

/// Create a new configuration source from a list of assignments key=value
///
/// The function iterates over the list, and for each element, it tries to
/// (a) identify the key and the value, by searching for the '=' sign.
/// (b) parse the value into one of bool, i64, f64. if not it's a string.
pub fn config_from_args(args: impl IntoIterator<Item = String>) -> Result<Config, Error> {
    let mut config = Config::builder();

    for arg in args {
        let (key, val) = arg.split_once('=').ok_or(Error::Splitting {
            msg: format!("missing '=' in setting override: {}", arg),
        })?;

        config = {
            if let Ok(as_bool) = val.parse::<bool>() {
                config.set_override(key, as_bool).context(ConfigValue)
            } else if let Ok(as_int) = val.parse::<i64>() {
                config.set_override(key, as_int).context(ConfigValue)
            } else if let Ok(as_float) = val.parse::<f64>() {
                config.set_override(key, as_float).context(ConfigValue)
            } else {
                config.set_override(key, val).context(ConfigValue)
            }
        }?
    }

    config.build().context(ConfigCompilation)
}

pub fn load_es_config_for<D: ContainerDocument>(
    mappings: Option<PathBuf>,
    settings: Option<PathBuf>,
    args_override: Vec<String>,
    dataset: String,
) -> Result<Config, Error> {
    let mut cfg_builder = Config::builder().add_source(D::default_es_container_config());

    let config_dataset = config::Config::builder()
        .set_override("container.dataset", dataset)
        .unwrap()
        .build()
        .context(ConfigCompilation)?;

    cfg_builder = cfg_builder.add_source(config_dataset);

    if let Some(mappings) = mappings {
        cfg_builder = cfg_builder.add_source(config::File::from(mappings))
    }

    if let Some(settings) = settings {
        cfg_builder = cfg_builder.add_source(config::File::from(settings));
    }

    cfg_builder
        .add_source(config_from_args(args_override)?)
        .build()
        .context(ConfigCompilation)
}

// This function produces a new configuration for bragi based on command line arguments,
// configuration files, and environment variables.
// * For bragi, up to three configuration files are read. These files are all in a directory
//   given by the 'config dir' command line argument.
// * The first configuration file is 'default.toml'
// * The second depends on the run mode (eg test, dev, prod). The run mode can be specified
//   either by the command line, or with the RUN_MODE environment variable. Given a run mode,
//   we look for the corresponding file in the config directory: 'dev' -> 'config/dev.toml'.
//   Default values are overriden by this mode config file.
// * Finally we look for a 'config/local.toml' file which can still override previous values.
// * Any value in a config file can then be overriden by environment variable: For example
//   to replace service.port, we can specify XXX
// * There is a special treatment for:
//   - Elasticsearch URL, which is specified by ELASTICSEARCH_URL or ELASTICSEARCH_TEST_URL
//   - Bragi's web server's port and listening address can be specified by command line
//     arguments.
pub fn config_from<T: Into<Option<String>> + Clone>(
    config_dir: &Path,
    sub_dirs: &[&str],
    run_mode: T,
    prefix: &str,
) -> Result<Config, Error> {
    let mut builder = sub_dirs
        .iter()
        .fold(Config::builder(), |mut builder, sub_dir| {
            let dir_path = config_dir.join(sub_dir);

            let default_path = dir_path.join("default").with_extension("toml");
            builder = builder.add_source(File::from(default_path));

            // The RUN_MODE environment variable overides the one given as argument:
            if let Some(run_mode) = env::var("RUN_MODE")
                .ok()
                .or_else(|| run_mode.clone().into())
            {
                let run_mode_path = dir_path.join(&run_mode).with_extension("toml");
                builder = builder.add_source(File::from(run_mode_path).required(false));
            }

            // Add in a local configuration file
            // This file shouldn't be checked in to git
            let local_path = dir_path.join("local").with_extension("toml");
            builder = builder.add_source(File::from(local_path).required(false));
            builder
        });

    // Add in settings from the environment (with a prefix of OMS2MIMIR)
    // Eg.. `<prefix>_DEBUG=1 ./target/app` would set the `debug` key
    builder = builder.add_source(Environment::with_prefix(prefix).separator("_"));

    builder.build().context(ConfigCompilation)
}
