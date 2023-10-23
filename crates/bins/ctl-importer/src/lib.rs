use elastic_client::settings::ElasticsearchStorageConfig;
use exporter_config::MimirConfig;
use serde::Deserialize;
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

#[derive(Debug, clap::Parser)]
#[command(
name = "ctlmimir",
about = "Configure Elasticsearch Backend",
version = VERSION,
author = AUTHORS
)]
pub struct Opts {
    #[arg(short = 's', long = "setting", num_args = 0..)]
    pub settings: Vec<String>,
}
#[derive(Deserialize)]
pub struct CtlConfig {
    pub elasticsearch: ElasticsearchStorageConfig,
}

impl MimirConfig<'_> for CtlConfig {
    const ENV_PREFIX: &'static str = "MIMIR";

    fn file_sources() -> Vec<&'static str> {
        vec!["elasticsearch.toml"]
    }
}
