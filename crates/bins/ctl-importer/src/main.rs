use clap::Parser;

use ctl_importer::{CtlConfig, Opts};
use elastic_client::ElasticSearchClient;
use exporter_config::MimirConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let settings = CtlConfig::get(&opts.settings)?;

    tracing::info!(
        "Trying to connect to elasticsearch at {}",
        &settings.elasticsearch.url
    );

    let conn = ElasticSearchClient::conn(settings.elasticsearch).await?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    conn.update_templates().await?;
    Ok(())
}
