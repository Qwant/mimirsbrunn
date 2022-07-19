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

use clap::Parser;
use snafu::{ResultExt, Snafu};

use mimir::{adapters::secondary::elasticsearch, domain::ports::secondary::remote::Remote};
use mimirsbrunn::{settings::cosmogony2mimir as settings, utils::template::update_templates};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    Settings { source: settings::Error },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: mimir::domain::ports::secondary::remote::Error,
    },

    #[snafu(display("Execution Error {}", source))]
    Execution { source: Box<dyn std::error::Error> },

    #[snafu(display("Configuration Error {}", source))]
    Configuration { source: common::config::Error },

    #[snafu(display("Import Error {}", source))]
    Import { source: mimirsbrunn::admin::Error },
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
    tracing::info!(
        "Trying to connect to elasticsearch at {}",
        &settings.elasticsearch.url
    );
    let client = elasticsearch::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch)
        .await
        .context(ElasticsearchConnectionSnafu)
        .map_err(Box::new)?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    if settings.update_templates {
        update_templates(&client, opts.config_dir).await?;
    }

    tracing::info!("Indexing cosmogony from {:?}", &opts.input);

    mimirsbrunn::admin::index_cosmogony(
        &opts.input,
        settings.langs,
        &settings.container,
        settings.french_id_retrocompatibility,
        &client,
    )
    .await
    .context(ImportSnafu)
    .map_err(|err| Box::new(err) as Box<dyn snafu::Error>) // TODO Investigate why the need to cast?
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use futures::TryStreamExt;
    use serial_test::serial;

    use super::*;
    use mimir::{
        adapters::secondary::elasticsearch::{remote, ElasticsearchStorageConfig},
        domain::ports::primary::list_documents::ListDocuments,
        utils::docker,
    };
    use places::admin::Admin;

    #[tokio::test]
    #[serial]
    async fn should_return_an_error_when_given_an_invalid_es_url() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");
        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(),
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("elasticsearch.url='http://example.com:demo'")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony.json",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts);
        assert!(settings
            .unwrap_err()
            .to_string()
            .contains("invalid port number"));
    }

    #[tokio::test]
    #[serial]
    async fn should_return_an_error_when_given_an_url_not_es() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");
        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(),
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("elasticsearch.url='http://no-es.test'")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony.json",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Elasticsearch Connection Error"));
    }

    #[tokio::test]
    #[serial]
    async fn should_return_an_error_when_given_an_invalid_path_for_config() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR")].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony.json",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts);
        assert!(settings
            .unwrap_err()
            .to_string()
            .contains("Config Source Error"));
    }

    #[tokio::test]
    #[serial]
    async fn should_return_an_error_when_given_an_invalid_path_for_input() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: [env!("CARGO_MANIFEST_DIR"), "invalid.jsonl.gz"]
                .iter()
                .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Cosmogony Error: No such file or directory (os error 2)"));
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_override_some_settings() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");
        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(),
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("elasticsearch.wait_for_active_shards=1")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony.json",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).expect("settings");
        assert_eq!(settings.elasticsearch.wait_for_active_shards, 1);
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_a_small_cosmogony_file() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony",
                "bretagne.small.jsonl.gz",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let _res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_test_pool()
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();

        assert_eq!(admins.len(), 8);
        assert!(admins.iter().all(|admin| admin.boundary.is_some()));
        assert!(admins.iter().all(|admin| admin.coord.is_valid()));
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_cosmogony_with_langs() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("langs=['fr', 'en']")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony",
                "bretagne.small.jsonl.gz",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let _res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_test_pool()
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();

        let brittany = admins.iter().find(|a| a.name == "Bretagne").unwrap();
        assert_eq!(brittany.names.get("fr"), Some("Bretagne"));
        assert_eq!(brittany.names.get("en"), Some("Brittany"));
        assert_eq!(brittany.labels.get("en"), Some("Brittany"));
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_with_correct_values() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("langs=['fr', 'en']")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony",
                "bretagne.small.jsonl.gz",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let _res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_test_pool()
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();

        let brittany = admins.iter().find(|a| a.name == "Bretagne").unwrap();
        assert_eq!(brittany.id, "admin:osm:relation:102740");
        assert_eq!(brittany.zone_type, Some(cosmogony::ZoneType::State));
        assert_relative_eq!(brittany.weight, 0.002_298, epsilon = 1e-6);
        assert_eq!(
            brittany.codes,
            vec![
                ("ISO3166-2", "FR-BRE"),
                ("ref:INSEE", "53"),
                ("ref:nuts", "FRH;FRH0"),
                ("ref:nuts:1", "FRH"),
                ("ref:nuts:2", "FRH0"),
                ("wikidata", "Q12130")
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
        )
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_activate_french_id_retrocompatibility() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("french_id_retrocompatibility=true")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony",
                "limousin",
                "limousin.jsonl.gz",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let _res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_test_pool()
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();
        for adm_name in [
            "Saint-Sulpice-les-Champs",
            "Queyssac-les-Vignes",
            "Saint-Quentin-la-Chabanne",
        ] {
            let admin = admins.iter().find(|a| a.name == adm_name).unwrap();
            assert_eq!(admin.id, format!("admin:fr:{}", admin.insee));
        }
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_deactivate_french_id_retrocompatibility() {
        docker::initialize()
            .await
            .expect("elasticsearch docker initialization");

        let opts = settings::Opts {
            config_dir: [env!("CARGO_MANIFEST_DIR"), "config"].iter().collect(), // Not a valid config base dir
            run_mode: Some("testing".to_string()),
            settings: vec![String::from("french_id_retrocompatibility=false")],
            input: [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "fixtures",
                "cosmogony",
                "limousin",
                "limousin.jsonl.gz",
            ]
            .iter()
            .collect(),
            cmd: settings::Command::Run,
        };

        let settings = settings::Settings::new(&opts).unwrap();
        let _res = mimirsbrunn::utils::launch::launch_async(move || run(opts, settings)).await;

        // Now we query the index we just created. Since it's a small cosmogony file with few entries,
        // we'll just list all the documents in the index, and check them.
        let config = ElasticsearchStorageConfig::default_testing();

        let client = remote::connection_test_pool()
            .conn(config)
            .await
            .expect("Elasticsearch Connection Established");

        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();
        for adm_name in [
            "Saint-Sulpice-les-Champs",
            "Queyssac-les-Vignes",
            "Saint-Quentin-la-Chabanne",
        ] {
            let admin = admins.iter().find(|a| a.name == adm_name).unwrap();
            assert!(admin.id.starts_with("admin:osm:relation"));
        }
    }
}
