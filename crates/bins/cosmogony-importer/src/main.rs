// Copyright © 2016, Hove and/or its affiliates. All rights reserved.
//
// This file is part of Navitia,
//     the software to build cool stuff with public transport.
//
// Hope you'll enjoy and contribute to this project,
//     powered by Hove (www.kisio.com).
// Help us simplify mobility and open public transport:
//     a non ending quest to the responsive locomotion way of traveling!
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

use cosmogony_importer::{CosmogonySettings, Opts};
use elastic_client::ElasticSearchClient;
use exporter_config::MimirConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let settings = CosmogonySettings::get(&opts.settings)?;
    run(opts, settings).await?;
    Ok(())
}

async fn run(opts: Opts, settings: CosmogonySettings) -> anyhow::Result<()> {
    tracing::info!(
        "Trying to connect to elasticsearch at {}",
        &settings.elasticsearch.url
    );
    let client = ElasticSearchClient::conn(settings.elasticsearch).await?;

    tracing::info!("Connected to elasticsearch.");

    // Update all the template components and indexes
    if settings.update_templates {
        client.update_templates().await?;
    }

    tracing::info!("Indexing cosmogony from {:?}", &opts.input);

    lib_geo::admin::index_cosmogony(
        &opts.input,
        settings.langs,
        &settings.container,
        settings.french_id_retrocompatibility,
        &client,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use approx::assert_relative_eq;
    use futures::TryStreamExt;

    use places::admin::Admin;
    use serial_test::serial;
    use test_containers::ElasticSearchContainer;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn should_return_an_error_when_given_an_invalid_es_url() -> anyhow::Result<()> {
        let opts = Opts {
            settings: vec![String::from("elasticsearch.url='http://example.com:demo'")],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony.json"),
        };

        let settings = CosmogonySettings::get(&opts.settings);
        assert!(settings
            .unwrap_err()
            .to_string()
            .contains("invalid port number"));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_a_small_cosmogony_file() -> anyhow::Result<()> {
        let elastic_client = ElasticSearchContainer::start_and_build_client().await?;

        let opts = Opts {
            settings: vec![format!("elasticsearch.url='{}'", elastic_client.config.url)],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony/bretagne.small.jsonl.gz"),
        };
        let settings = CosmogonySettings::get(&opts.settings)?;
        run(opts, settings.clone()).await?;

        let client = ElasticSearchClient::conn(settings.elasticsearch).await?;
        let admins: Vec<Admin> = client.list_documents().await?.try_collect().await?;

        assert_eq!(admins.len(), 8);
        assert!(admins.iter().all(|admin| admin.boundary.is_some()));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_correctly_index_cosmogony_with_langs() -> anyhow::Result<()> {
        let elastic_client = ElasticSearchContainer::start_and_build_client().await?;

        let opts = Opts {
            settings: vec![
                "langs=['fr', 'en']".to_string(),
                format!("elasticsearch.url='{}'", elastic_client.config.url),
            ],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony/bretagne.small.jsonl.gz"),
        };

        let settings = CosmogonySettings::get(&opts.settings.clone()).unwrap();
        run(opts, settings.clone()).await?;

        let client = ElasticSearchClient::conn(settings.elasticsearch).await?;

        let admins: Vec<Admin> = client.list_documents().await?.try_collect().await?;

        let brittany = admins.iter().find(|a| a.name == "Bretagne").unwrap();
        assert_eq!(brittany.names.get("fr"), Some("Bretagne"));
        assert_eq!(brittany.names.get("en"), Some("Brittany"));
        assert_eq!(brittany.labels.get("en"), Some("Brittany"));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_with_correct_values() -> anyhow::Result<()> {
        let elastic_client = ElasticSearchContainer::start_and_build_client().await?;

        let opts = Opts {
            settings: vec![
                "langs=['fr', 'en']".to_string(),
                format!("elasticsearch.url='{}'", elastic_client.config.url),
            ],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony/bretagne.small.jsonl.gz"),
        };

        let settings = CosmogonySettings::get(&opts.settings).unwrap();
        run(opts, settings.clone()).await?;

        let client = ElasticSearchClient::conn(settings.elasticsearch).await?;

        let admins: Vec<Admin> = client.list_documents().await?.try_collect().await?;

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
                ("wikidata", "Q12130"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
        );

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_activate_french_id_retro_compatibility() -> anyhow::Result<()> {
        let elastic_client = ElasticSearchContainer::start_and_build_client().await?;

        let opts = Opts {
            settings: vec![
                "french_id_retrocompatibility=true".to_string(),
                format!("elasticsearch.url='{}'", elastic_client.config.url),
            ],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony/limousin/limousin.jsonl.gz"),
        };

        let settings = CosmogonySettings::get(&opts.settings).unwrap();
        run(opts, settings.clone()).await?;

        let client = ElasticSearchClient::conn(settings.elasticsearch).await?;

        let admins: Vec<Admin> = client.list_documents().await?.try_collect().await?;

        for adm_name in [
            "Saint-Sulpice-les-Champs",
            "Queyssac-les-Vignes",
            "Saint-Quentin-la-Chabanne",
        ] {
            let admin = admins.iter().find(|a| a.name == adm_name).unwrap();
            assert_eq!(admin.id, format!("admin:fr:{}", admin.insee));
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn should_index_cosmogony_deactivate_french_id_retrocompatibility() -> anyhow::Result<()>
    {
        let elastic_client = ElasticSearchContainer::start_and_build_client().await?;

        let opts = Opts {
            settings: vec![
                "french_id_retrocompatibility=false".to_string(),
                format!("elasticsearch.url='{}'", elastic_client.config.url),
            ],
            input: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/cosmogony/limousin/limousin.jsonl.gz"),
        };

        let settings = CosmogonySettings::get(&opts.settings).unwrap();
        run(opts, settings.clone()).await?;
        let client = ElasticSearchClient::conn(settings.elasticsearch).await?;

        let admins: Vec<Admin> = client.list_documents().await?.try_collect().await?;

        for adm_name in [
            "Saint-Sulpice-les-Champs",
            "Queyssac-les-Vignes",
            "Saint-Quentin-la-Chabanne",
        ] {
            let admin = admins.iter().find(|a| a.name == adm_name).unwrap();
            assert!(admin.id.starts_with("admin:osm:relation"));
        }

        Ok(())
    }
}
