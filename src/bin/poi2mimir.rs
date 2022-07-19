use clap::Parser;
use snafu::{ResultExt, Snafu};

use mimir::{adapters::secondary::elasticsearch, domain::ports::secondary::remote::Remote};
use mimirsbrunn::{settings::poi2mimir as settings, utils::template::update_templates};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    Settings { source: settings::Error },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: mimir::domain::ports::secondary::remote::Error,
    },

    #[snafu(display("Configuration Error {}", source))]
    Configuration { source: common::config::Error },

    #[snafu(display("Execution Error {}", source))]
    Execution { source: Box<dyn std::error::Error> },
}

fn main() -> Result<(), Error> {
    let opts = settings::Opts::parse();
    let settings = settings::Settings::new(&opts).context(SettingsSnafu)?;

    match opts.cmd {
        settings::Command::Run => mimirsbrunn::utils::launch::launch_with_runtime(
            settings.nb_threads,
            run(opts, settings),
        )
        .context(ExecutionSnafu),
        settings::Command::Config => {
            println!("{}", serde_json::to_string_pretty(&settings).unwrap());
            Ok(())
        }
    }
}

async fn run(
    opts: settings::Opts,
    settings: settings::Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = elasticsearch::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch.clone())
        .await
        .context(ElasticsearchConnectionSnafu)?;

    // Update all the template components and indexes
    if settings.update_templates {
        update_templates(&client, opts.config_dir).await?;
    }

    mimirsbrunn::pois::index_pois(opts.input, &client, settings).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use serial_test::serial;

    use super::*;
    use ::tests::{bano, cosmogony, osm};
    use mimir::{
        adapters::secondary::elasticsearch::{remote, ElasticsearchStorageConfig},
        domain::ports::primary::list_documents::ListDocuments,
        utils::docker,
    };
    use mimirsbrunn::settings::poi2mimir as settings;
    use places::poi::Poi;

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_poi_file() {
        docker::initialize()
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
        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(),
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "poi",
                "limousin.poi",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();

        mimirsbrunn::utils::launch::launch_async(move || run(opts, settings))
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
