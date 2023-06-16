use clap::Parser;
use elastic_client::model::configuration::ContainerConfig;
use futures::stream::StreamExt;
use snafu::{ResultExt, Snafu};
use tracing::instrument;

use elastic_client::remote::Remote;
use elastic_client::{self, ElasticsearchStorage};
use lib_geo::admin_geofinder::AdminGeoFinder;
use lib_geo::osm_reader::street::streets;
use lib_geo::settings::admin_settings::AdminSettings;
use lib_geo::utils::template::update_templates;
use osm_importer::{Command, ConfigError, Opts, Settings};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Settings (Configuration or CLI) Error: {}", source))]
    Settings { source: ConfigError },

    #[snafu(display("OSM PBF Reader Error: {}", source))]
    OsmPbfReader { source: lib_geo::osm_reader::Error },

    #[snafu(display("Elasticsearch Connection Pool {}", source))]
    ElasticsearchConnection {
        source: elastic_client::remote::RemoteError,
    },

    #[snafu(display("Street Extraction from OSM PBF Error {}", source))]
    StreetOsmExtraction {
        source: lib_geo::osm_reader::street::Error,
    },

    #[snafu(display("Street Index Creation Error {}", source))]
    StreetIndexCreation {
        source: elastic_client::model::error::Error,
    },

    #[snafu(display("Poi Extraction from OSM PBF Error {}", source))]
    PoiOsmExtraction {
        source: lib_geo::osm_reader::poi::Error,
    },

    #[snafu(display("Poi Index Creation Error {}", source))]
    PoiIndexCreation {
        source: elastic_client::model::error::Error,
    },

    #[snafu(display("Elasticsearch Configuration {}", source))]
    StreetElasticsearchConfiguration { source: exporter_config::Error },

    #[snafu(display("Execution Error {}", source))]
    Execution { source: Box<dyn std::error::Error> },

    #[snafu(display("Admin Retrieval Error {}", details))]
    AdminRetrieval { details: String },
}

fn main() -> Result<(), Error> {
    let opts = crate::Opts::parse();
    let settings = crate::Settings::new(&opts).context(SettingsSnafu)?;

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
    let mut osm_reader =
        lib_geo::osm_reader::make_osm_reader(&opts.input).context(OsmPbfReaderSnafu)?;

    let client = elastic_client::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch.clone())
        .await
        .context(ElasticsearchConnectionSnafu)?;

    // Update all the template components and indexes
    if settings.update_templates {
        update_templates(&client, opts.config_dir).await?;
    }

    let admin_settings = AdminSettings::build(&settings.admins);

    let admins_geofinder = AdminGeoFinder::build(&admin_settings, &client).await?;

    if settings.streets.import {
        let streets = streets(
            &mut osm_reader,
            &admins_geofinder,
            &settings.streets.exclusions,
            #[cfg(feature = "db-storage")]
            settings.database.as_ref(),
        )
        .context(StreetOsmExtractionSnafu)?;

        import_streets(streets, &client, &settings.container_street).await?;
    }

    if settings.pois.import {
        import_pois(
            &mut osm_reader,
            &admins_geofinder,
            &settings.pois.config.clone().unwrap_or_default(),
            &client,
            &settings.container_poi,
            settings.pois.max_distance_reverse,
        )
        .await?;
    }

    Ok(())
}

#[instrument(skip_all)]
async fn import_streets(
    streets: Vec<places::street::Street>,
    client: &ElasticsearchStorage,
    config: &ContainerConfig,
) -> Result<(), Error> {
    let streets = streets
        .into_iter()
        .map(|street| street.set_weight_from_admins());

    let _index = client
        .generate_index(config, futures::stream::iter(streets))
        .await
        .context(StreetIndexCreationSnafu)?;

    Ok(())
}

#[instrument(skip_all)]
async fn import_pois(
    osm_reader: &mut lib_geo::osm_reader::OsmPbfReader,
    admins_geofinder: &AdminGeoFinder,
    poi_config: &lib_geo::osm_reader::poi::PoiConfig,
    client: &ElasticsearchStorage,
    config: &ContainerConfig,
    max_distance_reverse: usize,
) -> Result<(), Error> {
    // This function rely on AdminGeoFinder::get_objs_and_deps
    // which use all available cpu/cores to decode osm file and cannot be limited by tokio runtime
    let pois = lib_geo::osm_reader::poi::pois(osm_reader, poi_config, admins_geofinder)
        .context(PoiOsmExtractionSnafu)?;

    let pois: Vec<places::poi::Poi> = futures::stream::iter(pois)
        .map(lib_geo::osm_reader::poi::compute_weight)
        .then(|poi| {
            lib_geo::osm_reader::poi::add_address(
                &client.config.index_root,
                client,
                poi,
                max_distance_reverse,
            )
        })
        .collect()
        .await;

    let _ = client
        .generate_index(config, futures::stream::iter(pois))
        .await
        .context(PoiIndexCreationSnafu)?;

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use elastic_client::ElasticsearchStorage;
//     use exporter_config::load_es_config_for;
//     use places::ContainerDocument;
//     use futures::TryStreamExt;
//     use elastic_client::model::query::Query;
//     use elastic_client::ports::primary::list_documents::ListDocuments;
//     use elastic_client::ports::primary::search_documents::SearchDocuments;
//     use mimir::{adapters::secondary::elastic_client::remote, utils::docker};
//     use lib_geo::admin::index_cosmogony;
//     use places::{admin::Admin, street::Street, Place};
//     use serial_test::serial;
//     use structopt::StructOpt;
//
//     fn elasticsearch_test_url() -> String {
//         std::env::var(elastic_client::remote::ES_TEST_KEY).expect("env var")
//     }
//
//     async fn index_cosmogony_admins(client: &ElasticsearchStorage) {
//         index_cosmogony(
//             "./tests/fixtures/cosmogony.json".into(),
//             vec![],
//             load_es_config_for(
//                 None,
//                 None,
//                 vec!["container.dataset=osm2mimir-test".into()],
//                 String::from("fr"),
//             )
//             .unwrap(),
//             client,
//         )
//         .await
//         .unwrap()
//     }
//
//     #[tokio::test]
//     #[serial]
//     async fn should_correctly_index_osm_streets_and_pois() {
//         test_containers::initialize()
//             .await
//             .expect("elasticsearch docker initialization");
//
//         // Now we query the index we just created. Since it's a small cosmogony file with few entries,
//         // we'll just list all the documents in the index, and check them.
//         let pool = remote::connection_test_pool()
//             .await
//             .expect("Elasticsearch Connection Pool");
//
//         let client = pool
//             .conn(ES_DEFAULT_TIMEOUT, ES_DEFAULT_VERSION_REQ)
//             .await
//             .expect("Elasticsearch Connection Established");
//
//         index_cosmogony_admins(&client).await;
//
//         let storage_args = if cfg!(feature = "db-storage") {
//             vec!["--db-file=test-db.sqlite3", "--db-buffer-size=10"]
//         } else {
//             vec![]
//         };
//
//         let args = Args::from_iter(
//             [
//                 "osm2mimir",
//                 "--input=./tests/fixtures/osm_fixture.osm.pbf",
//                 "--dataset=osm2mimir-test",
//                 "--import-way=true",
//                 "--import-poi=true",
//                 &format!("-c={}", elasticsearch_test_url()),
//             ]
//             .iter()
//             .copied()
//             .chain(storage_args),
//         );
//
//         let _res = runtime_async_args(run, args).await;
//
//         let search = |query: &str| {
//             let client = client.clone();
//             let query: String = query.into();
//             async move {
//                 client
//                     .search_documents(
//                         vec![
//                             Street::static_doc_type().into(),
//                             Poi::static_doc_type().into(),
//                         ],
//                         Query::QueryString(format!("full_label.prefix:({})", query)),
//                     )
//                     .await
//                     .unwrap()
//                     .into_iter()
//                     .map(|json| serde_json::from_value::<Place>(json).unwrap())
//                     .collect::<Vec<Place>>()
//             }
//         };
//
//         let streets: Vec<Street> = client
//             .list_documents()
//             .await
//             .unwrap()
//             .try_collect()
//             .await
//             .unwrap();
//         assert_eq!(streets.len(), 13);
//
//         // Basic street search
//         let results = search("Rue des Près").await;
//         assert_eq!(results[0].label(), "Rue des Près (Livry-sur-Seine)");
//         assert_eq!(
//             results
//                 .iter()
//                 .filter(
//                     |place| place.is_street() && place.label() == "Rue des Près (Livry-sur-Seine)"
//                 )
//                 .count(),
//             1,
//             "Only 1 'Rue des Près' is expected"
//         );
//
//         // All ways with same name in the same city are merged into a single street
//         let results = search("Rue du Four à Chaux").await;
//         assert_eq!(
//             results.iter()
//                 .filter(|place| place.label() == "Rue du Four à Chaux (Livry-sur-Seine)")
//                 .count(),
//             1,
//             "Only 1 'Rue du Four à Chaux' is expected as all ways the same name should be merged into 1 street."
//         );
//         assert_eq!(
//             results[0].id(),
//             "street:osm:way:40812939",
//             "The way with minimum way_id should be used as street id."
//         );
//
//         // Street admin is based on a middle node.
//         // (Here the first node is located outside Melun)
//         let results = search("Rue Marcel Houdet").await;
//         assert_eq!(results[0].label(), "Rue Marcel Houdet (Melun)");
//         assert!(results[0]
//             .admins()
//             .iter()
//             .filter(|a| a.is_city())
//             .any(|a| a.name == "Melun"));
//
//         // Basic search for Poi by label
//         let res = search("Le-Mée-sur-Seine Courtilleraies").await;
//         assert_eq!(
//             res[0].poi().expect("Place should be a poi").poi_type.id,
//             "poi_type:amenity:post_office"
//         );
//
//         // highway=bus_stop should not be indexed
//         let res = search("Grand Châtelet").await;
//         assert!(
//             res.is_empty(),
//             "'Grand Châtelet' (highway=bus_stop) should not be found."
//         );
//
//         // "Rue de Villiers" is at the exact neighborhood between two cities, a
//         // document must be added for both.
//         let results = search("Rue de Villiers").await;
//         assert!(["Neuilly-sur-Seine", "Levallois-Perret"]
//             .iter()
//             .all(|city| {
//                 results.iter().any(|poi| {
//                     poi.admins()
//                         .iter()
//                         .filter(|a| a.is_city())
//                         .any(|admin| &admin.name == city)
//                 })
//             }));
//     }
// }
