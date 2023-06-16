use clap::Parser;
use snafu::{ResultExt, Snafu};

use crate::pois::index_pois;
use elastic_client::remote::Remote;
use lib_geo::utils::template::update_templates;
use poi_importer::{Command, ConfigError, Opts, Settings};

mod pois;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    Settings { source: ConfigError },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: elastic_client::remote::RemoteError,
    },

    #[snafu(display("Configuration Error {}", source))]
    Configuration { source: exporter_config::Error },

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
    let client = elastic_client::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch.clone())
        .await
        .context(ElasticsearchConnectionSnafu)?;

    // Update all the template components and indexes
    if settings.update_templates {
        update_templates(&client, opts.config_dir).await?;
    }

    index_pois(opts.input, &client, settings).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use serial_test::serial;
    use std::path::PathBuf;

    use super::*;
    use elastic_client::{remote, ElasticsearchStorageConfig};
    use exporter_config::CONFIG_PATH;
    use places::poi::Poi;
    use test_harness::{bano, cosmogony, osm};

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_poi_file() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        // We need to prep the test by inserting admins, addresses, and streets.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_pool_url(&config.url)
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        cosmogony::index_admins(&client, "limousin", "limousin", true, true)
            .await
            .unwrap();

        osm::index_streets(&client, "limousin", "limousin", true)
            .await
            .unwrap();

        bano::index_addresses(&client, "limousin", "limousin", true)
            .await
            .unwrap();

        // And here is the indexing of Pois...
        let opts = Opts {
            config_dir: CONFIG_PATH.into(),
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/poi/limousin.poi"),
            cmd: Command::Run,
        };

        let settings = Settings::new(&opts).unwrap();

        runtime::launch_async(move || run(opts, settings))
            .await
            .unwrap();

        // Now we query the index we just created. Since it's a small poi file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_pool_url(&config.url)
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let pois: Vec<Poi> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(pois.len(), 1);
    }
}
