// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
//
// LICENCE: This program is free software; you can redistribute it
// and/or modify it under the terms of the GNU Affero General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public
// License along with this program. If not, see
// <http://www.gnu.org/licenses/>.
//
// Stay tuned using
// twitter @navitia
// IRC #navitia on freenode
// https://groups.google.com/d/forum/navitia
// www.navitia.io

use bano_importer::{Command, ConfigError, Opts, Settings};
use clap::Parser;
use lib_geo::addr_reader::import_addresses_from_input_path;
use lib_geo::admin_geofinder::AdminGeoFinder;
use lib_geo::settings::admin_settings::AdminSettings;
use lib_geo::utils::template::update_templates;
use snafu::{ResultExt, Snafu};

use elastic_client::remote::Remote;
use lib_geo::bano::Bano;

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

    #[snafu(display("Configuration Error {}", source))]
    Configuration { source: exporter_config::Error },

    #[snafu(display("Index Creation Error {}", source))]
    IndexCreation {
        source: elastic_client::model::error::Error,
    },

    #[snafu(display("Admin Retrieval Error {}", details))]
    AdminRetrieval { details: String },
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
        .conn(settings.elasticsearch)
        .await
        .context(ElasticsearchConnectionSnafu)
        .map_err(Box::new)?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    if settings.update_templates {
        update_templates(&client, opts.config_dir).await?;
    }

    // TODO There might be an opportunity for optimization here:
    // Lets say we're indexing a single bano department.... we don't need to retrieve
    // the admins for other regions!
    let into_addr = {
        let admin_settings = AdminSettings::build(&settings.admins);
        let admins_geofinder = AdminGeoFinder::build(&admin_settings, &client).await?;

        let admins_by_insee = admins_geofinder
            .iter()
            .filter(|a| !a.insee.is_empty())
            .map(|a| (a.insee.clone(), a))
            .collect();

        move |b: Bano| b.into_addr(&admins_by_insee, &admins_geofinder)
    };

    let addresses = import_addresses_from_input_path(opts.input, false, into_addr)
        .await
        .map_err(Box::new)?;

    client
        .generate_index(&settings.container, addresses)
        .await
        .context(IndexCreationSnafu)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use bano_importer::Settings;
    use elastic_client::{remote, ElasticsearchStorageConfig};
    use exporter_config::CONFIG_PATH;
    use futures::TryStreamExt;
    use lib_geo::settings::admin_settings::AdminFromCosmogonyFile;
    use places::addr::Addr;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_bano_file() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = Opts {
            config_dir: CONFIG_PATH.into(),
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/sample-bano.csv"),
            cmd: Command::Run,
        };

        let mut settings = Settings::new(&opts).unwrap();
        let cosmogony_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/cosmogony/ile-de-france/ile-de-france.jsonl.gz");

        settings.admins = Some(AdminFromCosmogonyFile {
            french_id_retrocompatibility: false,
            langs: vec!["fr".to_string()],
            cosmogony_file,
        });

        runtime::launch_async(move || run(opts, settings))
            .await
            .unwrap();

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_pool_url(&config.url)
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let addresses: Vec<Addr> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(addresses.len(), 35);

        let addr1 = addresses
            .iter()
            .find(|&addr| addr.name == "10 Place de la Mairie")
            .unwrap();

        assert_eq!(addr1.id, "addr:1.378886;43.668175:10");

        let addr2 = addresses
            .iter()
            .find(|&addr| addr.name == "999 Rue Foncet")
            .unwrap();

        assert_eq!(addr2.zip_codes, vec!["06000", "06100", "06200", "06300"]);
    }

    #[tokio::test]
    #[serial]
    async fn should_fail_on_invalid_path() {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = Opts {
            config_dir: CONFIG_PATH.into(),
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: "does-not-exist.csv".into(),
            cmd: Command::Run,
        };

        let settings = Settings::new(&opts).unwrap();
        let res = runtime::launch_async(move || run(opts, settings)).await;
        assert!(res.is_err());
    }
}
