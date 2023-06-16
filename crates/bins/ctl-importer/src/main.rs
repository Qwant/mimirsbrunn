use clap::Parser;
use ctl_importer::{Command, ConfigError, Opts, Settings};
use elastic_client::remote::Remote;
use lib_geo::utils::template::update_templates;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    Settings { source: ConfigError },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: elastic_client::remote::RemoteError,
    },

    #[snafu(display("Execution Error {}", source))]
    Execution { source: Box<dyn std::error::Error> },
}

fn main() -> Result<(), Error> {
    let opts = Opts::parse();
    let settings = Settings::new(&opts).context(SettingsSnafu)?;

    match opts.cmd {
        Command::Run => runtime::launch_with_runtime(settings.nb_threads, run(opts, settings))
            .context(ExecutionSnafu),
        Command::Config => {
            println!("{}", serde_json::to_string_pretty(&settings).unwrap());
            Ok(())
        }
    }
}

async fn run(opts: Opts, settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(
        "Trying to connect to elasticsearch at {}",
        &settings.elasticsearch.url
    );

    let client = elastic_client::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch)
        .await
        .context(ElasticsearchConnectionSnafu)
        .map_err(Box::new)?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    update_templates(&client, opts.config_dir).await?;
    Ok(())
}
