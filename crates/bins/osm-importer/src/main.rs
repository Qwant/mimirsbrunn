use clap::Parser;
use elastic_client::model::configuration::ContainerConfig;
use futures::stream::StreamExt;
use tracing::instrument;

use elastic_client::remote::Remote;
use elastic_client::{self, ElasticsearchStorage};
use lib_geo::admin_geofinder::AdminGeoFinder;
use lib_geo::osm_reader::street::streets;
use lib_geo::settings::admin_settings::AdminSettings;
use lib_geo::utils::template::update_templates;
use osm_importer::{Opts, Settings};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let settings = Settings::new(&opts)?;

    let mut osm_reader = lib_geo::osm_reader::make_osm_reader(&opts.input)?;

    let client = elastic_client::remote::connection_pool_url(&settings.elasticsearch.url)
        .conn(settings.elasticsearch.clone())
        .await?;

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
        )?;

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
) -> anyhow::Result<()> {
    let streets = streets
        .into_iter()
        .map(|street| street.set_weight_from_admins());

    client
        .generate_index(config, futures::stream::iter(streets))
        .await?;

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
) -> anyhow::Result<()> {
    // This function rely on AdminGeoFinder::get_objs_and_deps
    // which use all available cpu/cores to decode osm file and cannot be limited by tokio runtime
    let pois = lib_geo::osm_reader::poi::pois(osm_reader, poi_config, admins_geofinder)?;

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

    client
        .generate_index(config, futures::stream::iter(pois))
        .await?;

    Ok(())
}
