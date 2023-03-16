use config::{Config, Environment, File};
use snafu::{ResultExt, Snafu};
use std::{env, path::Path};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Key Value Splitting Error: {}", msg))]
    Splitting { msg: String },

    #[snafu(display("Setting Config Value Error: {}", source))]
    ConfigValue { source: config::ConfigError },

    #[snafu(display("Config Compilation Error: {}", source))]
    ConfigCompilation { source: config::ConfigError },

    #[snafu(display("Unrecognized Value Type Error: {}", details))]
    UnrecognizedValueType { details: String },
}

// This function produces a new configuration based on the arguments:
// In the base directory (`config_dir`), it will look for the subdirectories.
// * In each subdirectory, it will look for a default configuration file.
// * If a run_mode is provided, then the corresponding file is sourced in each of these
//   subdirectory.
// * Then, if a prefix is given, we source environment variables starting with
//   that string, if it is not given it defaults to the `MIMIR` prefix.
// * And finally, we can make manual adjusts with a list of key value pairs.
pub fn config_from<
    'a,
    R: Into<Option<&'a str>> + Clone,
    P: Into<Option<&'a str>>,
    D: AsRef<str>,
>(
    config_dir: &Path,
    sub_dirs: &[D],
    run_mode: R,
    prefix: P,
    overrides: Vec<String>,
) -> Result<Config, Error> {
    let mut config = sub_dirs
        .iter()
        .try_fold(Config::default(), |mut config, sub_dir| {
            let dir_path = config_dir.join(sub_dir.as_ref());

            let default_path = dir_path.join("default").with_extension("toml");
            config.merge(File::from(default_path))?;

            // The RUN_MODE environment variable overides the one given as argument:
            if let Some(run_mode) = env::var("RUN_MODE")
                .ok()
                .or_else(|| run_mode.clone().into().map(String::from))
            {
                let run_mode_path = dir_path.join(run_mode).with_extension("toml");

                if run_mode_path.is_file() {
                    config.merge(File::from(run_mode_path).required(false))?;
                }
            }

            // Add in a local configuration file
            // This file shouldn't be checked in to git
            let local_path = dir_path.join("local").with_extension("toml");
            config.merge(File::from(local_path).required(false))?;
            Ok(config)
        })
        .context(ConfigCompilationSnafu)?;

    // Add in settings from the environment
    // Eg.. `<prefix>_DEBUG=1 ./target/app` would set the `debug` key
    if let Some(prefix) = prefix.into() {
        let prefix = Environment::with_prefix(prefix).separator("__");
        config.merge(prefix).context(ConfigCompilationSnafu)?;
    };

    // Add command line overrides
    if !overrides.is_empty() {
        config
            .merge(config_from_args(overrides)?)
            .context(ConfigCompilationSnafu)?;
    }

    Ok(config)
}

/// Create a new configuration source from a list of assignments key=value
fn config_from_args(args: impl IntoIterator<Item = String>) -> Result<Config, Error> {
    args.into_iter()
        .try_fold(Config::default(), |builder, arg| {
            builder.with_merged(config::File::from_str(&arg, config::FileFormat::Toml))
        })
        .context(ConfigCompilationSnafu)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn should_correctly_create_a_source_from_int_assignment() {
        let overrides = vec![String::from("foo=42")];
        let config = config_from_args(overrides).unwrap();
        let val: i32 = config.get("foo").unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn should_correctly_create_a_source_from_string_assignment() {
        let overrides = vec![String::from("foo='42'")];
        let config = config_from_args(overrides).unwrap();
        let val: String = config.get("foo").unwrap();
        assert_eq!(val, "42");
    }

    #[test]
    fn should_correctly_create_a_source_from_array_assignment() {
        let overrides = vec![String::from("foo=['fr', 'en']")];
        let config = config_from_args(overrides).unwrap();
        let val: Vec<String> = config.get("foo").unwrap();
        assert_eq!(val[0], "fr");
        assert_eq!(val[1], "en");
    }

    #[test]
    fn should_correctly_create_a_source_from_multiple_assignments() {
        let overrides = vec![
            String::from("elasticsearch.url='http://localhost:9200'"),
            String::from("service.port=6666"),
        ];
        let config = config_from_args(overrides).unwrap();
        let url: String = config.get("elasticsearch.url").unwrap();
        let port: i32 = config.get("service.port").unwrap();
        assert_eq!(url, "http://localhost:9200");
        assert_eq!(port, 6666);
    }
}
