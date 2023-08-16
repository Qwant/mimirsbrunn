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
