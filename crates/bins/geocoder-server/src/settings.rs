use std::env;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use elastic_client::settings::ElasticsearchStorageConfig;
use elastic_query_builder::settings::QuerySettings;
use exporter_config::MimirConfig;
use serde_helpers::deserialize_duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, clap::Parser)]
#[command(
name = "bragi",
about = "REST API for querying Elasticsearch",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BragiSettings {
    pub elasticsearch: ElasticsearchStorageConfig,
    pub query: QuerySettings,
    pub service: Service,
    pub http_cache_duration: usize,
    #[serde(deserialize_with = "deserialize_duration")]
    pub autocomplete_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub reverse_timeout: Duration,
    #[serde(deserialize_with = "deserialize_duration")]
    pub features_timeout: Duration,
}

impl MimirConfig<'_> for BragiSettings {
    const ENV_PREFIX: &'static str = "BRAGI";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml", "geocoder.toml", "query-config.toml"]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Host on which we expose bragi. Example: 'http://localhost', '0.0.0.0'
    pub host: String,
    /// Port on which we expose bragi.
    pub port: u16,
    /// Used on POST request to set an upper limit on the size of the body (in bytes)
    pub content_length_limit: u64,
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn should_override_config_with_env() -> anyhow::Result<()> {
        env::set_var("BRAGI__HTTP_CACHE_DURATION", "42");
        let settings = BragiSettings::get(&[])?;

        let cache_duration = settings.http_cache_duration;
        assert_that!(cache_duration).is_equal_to(42);

        Ok(())
    }

    #[test]
    fn should_override_nested_config_with_env() -> anyhow::Result<()> {
        env::set_var("BRAGI__ELASTICSEARCH__URL", "http://elastic.cool/");
        env::set_var("BRAGI__ELASTICSEARCH__INDEX_ROOT", "titi");
        let settings = BragiSettings::get(&[])?;

        let elastic_search_url = settings.elasticsearch.url.as_str();
        let index_root = settings.elasticsearch.index_root.as_str();
        assert_that!(elastic_search_url).is_equal_to("http://elastic.cool/");
        assert_that!(index_root).is_equal_to("titi");

        Ok(())
    }

    #[test]
    fn should_override_with_cli_args() -> anyhow::Result<()> {
        let settings =
            BragiSettings::get(&["elasticsearch.url='http://gomugomuno.search/'".to_string()])?;

        let elastic_search_url = settings.elasticsearch.url.as_str();
        assert_that!(elastic_search_url).is_equal_to("http://gomugomuno.search/");

        Ok(())
    }
}
