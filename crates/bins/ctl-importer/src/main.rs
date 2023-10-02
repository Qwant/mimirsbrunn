use clap::Parser;
use ctl_importer::{Opts, Settings};
use elastic_client::remote::Remote;
use lib_geo::utils::template::update_templates;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let settings = Settings::new(&opts)?;

    tracing::info!(
        "Trying to connect to elasticsearch at {}",
        &settings.elasticsearch.url
    );

    let client = elastic_client::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch)
        .await?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    update_templates(&client, opts.config_dir).await?;
    Ok(())
}
