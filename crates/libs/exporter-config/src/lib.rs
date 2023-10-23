use std::path::PathBuf;
use std::{env, io};

use config::{Config, Environment, File, FileFormat};
use serde::Deserialize;
use thiserror::Error;

const DEV_CONFIG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../config");

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Key Value Splitting Error: {}", msg)]
    Splitting { msg: String },

    #[error("Config error: {0}")]
    ConfigCompilation(#[from] config::ConfigError),

    #[error("IO error: {0}")]
    IOError(#[from] io::Error),

    #[error("Expected '=' separator in config override")]
    MalformedConfigOverride(String),
}

pub fn config_dir() -> PathBuf {
    let config_dir = PathBuf::from("/etc/mimirsbrunn/");
    if config_dir.exists() {
        config_dir
    } else {
        PathBuf::from(DEV_CONFIG_PATH)
    }
}

pub trait MimirConfig<'a>: Deserialize<'a> {
    const ENV_PREFIX: &'static str;

    fn file_sources() -> Vec<&'static str> {
        vec![]
    }
    fn root_key() -> Option<&'static str> {
        None
    }

    fn get(overrides: &[String]) -> Result<Self, ConfigError>
    where
        Self: Sized,
    {
        let mut override_env = vec![];
        for value in overrides {
            // If root key is present prepend the root key to the value override
            // Example: "url=http://localhost:9200" -> "elasticsearch.url=http://localhost:9200"
            let value = match Self::root_key() {
                None => value.clone(),
                Some(key) => format!("{key}.{value}"),
            };

            override_env.push(File::from_str(&value, FileFormat::Toml));
        }

        // Merge all config/* configs into a single Config struct
        let config_sources: Vec<File<_, _>> = Self::file_sources()
            .iter()
            .map(PathBuf::from)
            .map(|path| config_dir().join(path))
            .map(File::from)
            .collect();

        let config = Config::builder()
            .add_source(config_sources)
            .add_source(
                Environment::with_prefix(Self::ENV_PREFIX)
                    .separator("__")
                    .prefix_separator("__"),
            )
            .add_source(override_env);

        let config = config.build()?;

        match Self::root_key() {
            None => Ok(config.try_deserialize()?),
            Some(key) => {
                let config = config.get::<Self>(key)?;
                Ok(config)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use speculoos::assert_that;

    use super::*;

    #[derive(Deserialize, Debug)]
    pub struct TestSettings {
        foo: u32,
    }

    #[derive(Deserialize, Debug)]
    pub struct TestSettingsWithArray {
        foo: Vec<String>,
    }

    #[derive(Deserialize, Debug)]
    pub struct TestSettingsMultipleAssignments {
        url: String,
        inner: InnerSettings,
    }

    #[derive(Deserialize, Debug)]
    pub struct InnerSettings {
        port: u16,
    }

    impl MimirConfig<'_> for TestSettings {
        const ENV_PREFIX: &'static str = "TEST";
    }

    impl MimirConfig<'_> for TestSettingsWithArray {
        const ENV_PREFIX: &'static str = "TEST";
    }

    impl MimirConfig<'_> for TestSettingsMultipleAssignments {
        const ENV_PREFIX: &'static str = "TEST";
    }

    #[test]
    fn should_correctly_create_a_source_from_int_assignment() -> anyhow::Result<()> {
        let overrides = vec!["foo=42".to_string()];
        let config = TestSettings::get(&overrides)?;
        assert_that!(config).map(|c| &c.foo).is_equal_to(42);
        Ok(())
    }

    #[test]
    fn should_correctly_create_a_source_from_string_assignment() -> anyhow::Result<()> {
        let overrides = vec!["foo=42".to_string()];
        let config = TestSettings::get(&overrides)?;
        assert_that!(config).map(|c| &c.foo).is_equal_to(42);
        Ok(())
    }

    #[test]
    fn should_correctly_create_a_source_from_array_assignment() -> anyhow::Result<()> {
        let overrides = vec![String::from("foo=[ 'fr','en' ]")];
        let config = TestSettingsWithArray::get(&overrides)?;
        assert_that!(config)
            .map(|c| &c.foo)
            .is_equal_to(vec!["fr".to_string(), "en".to_string()]);
        Ok(())
    }

    #[test]
    fn should_correctly_create_a_source_from_multiple_assignments() -> anyhow::Result<()> {
        let overrides = vec![
            String::from("url='http://localhost:9200'"),
            String::from("inner.port=6666"),
        ];
        let config = TestSettingsMultipleAssignments::get(&overrides)?;
        assert_that!(config.url).is_equal_to("http://localhost:9200".to_string());
        assert_that!(config.inner.port).is_equal_to(6666);
        Ok(())
    }
}
