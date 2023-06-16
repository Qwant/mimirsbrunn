use futures::stream::StreamExt;
use futures::TryStreamExt;
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

use elastic_client::model::error::Error as ModelError;
use elastic_client::storage::Error as StorageError;
use elastic_client::ElasticsearchStorage;
use elastic_query_builder::doc_type::root_doctype_dataset;
use exporter_config::CONFIG_PATH;
use lib_geo::admin_geofinder::AdminGeoFinder;
use places::poi::Poi;
use places::street::Street;
use places::ContainerDocument;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },

    #[snafu(display("OSM PBF Reader Error: {}", source))]
    OsmPbfReader { source: lib_geo::osm_reader::Error },
    #[snafu(display("Poi Extraction from OSM PBF Error {}", source))]
    PoiOsmExtraction {
        source: lib_geo::osm_reader::poi::Error,
    },

    #[snafu(display("Street Extraction from OSM PBF Error {}", source))]
    StreetOsmExtraction {
        source: lib_geo::osm_reader::street::Error,
    },

    #[snafu(display("Poi Index Creation Error {}", source))]
    PoiIndexCreation {
        source: elastic_client::model::error::Error,
    },

    #[snafu(display("List Document Error {}", source))]
    ListDocument {
        source: elastic_client::model::error::Error,
    },

    #[snafu(display("Could not get Config {}", source))]
    Config { source: exporter_config::Error },

    #[snafu(display("Invalid Configuration {}", source))]
    ConfigInvalid { source: config::ConfigError },
}

pub enum Status {
    Skipped,
    Done,
}

pub async fn index_pois(
    client: &ElasticsearchStorage,
    region: &str,
    dataset: &str,
    reindex_if_already_exists: bool,
) -> Result<Status, Error> {
    // Check if the address index already exists
    let container =
        root_doctype_dataset(&client.config.index_root, Poi::static_doc_type(), dataset);

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index BANO file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    let input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/osm/")
        .join(region)
        .join(format!("{}-latest.osm.pbf", region));

    let mut osm_reader =
        lib_geo::osm_reader::make_osm_reader(&input_file).context(OsmPbfReaderSnafu)?;

    let admins_geofinder: AdminGeoFinder = client
        .list_documents()
        .await
        .map_err(|err| ModelError::DocumentRetrievalError { source: err.into() })
        .context(ListDocumentSnafu)?
        .try_collect()
        .await
        .map_err(|err| ModelError::DocumentRetrievalError { source: err.into() })
        .context(ListDocumentSnafu)?;

    // Read the poi configuration from the osm2mimir configuration / testing mode.
    let config_dir: PathBuf = CONFIG_PATH.into();
    let mut config: osm_importer::Settings = exporter_config::config_from(
        &config_dir,
        &["osm2mimir", "elasticsearch"],
        "testing",
        None,
        vec![],
    )
    .context(ConfigSnafu)?
    .try_into()
    .context(ConfigInvalidSnafu)?;
    config.container_poi.dataset = dataset.to_string();
    config.container_street.dataset = dataset.to_string();

    let pois = lib_geo::osm_reader::poi::pois(
        &mut osm_reader,
        &config.pois.config.unwrap(),
        &admins_geofinder,
    )
    .context(PoiOsmExtractionSnafu)?;

    let pois: Vec<Poi> = futures::stream::iter(pois)
        .map(lib_geo::osm_reader::poi::compute_weight)
        .then(|poi| {
            lib_geo::osm_reader::poi::add_address(
                &client.config.index_root,
                client,
                poi,
                config.pois.max_distance_reverse,
            )
        })
        .collect()
        .await;
    let _ = client
        .generate_index(&config.container_poi, futures::stream::iter(pois))
        .await
        .context(PoiIndexCreationSnafu)?;

    Ok(Status::Done)
}

pub async fn index_streets(
    client: &ElasticsearchStorage,
    region: &str,
    dataset: &str,
    reindex_if_already_exists: bool,
) -> Result<Status, Error> {
    // Check if the address index already exists
    let container = root_doctype_dataset(
        &client.config.index_root,
        Street::static_doc_type(),
        dataset,
    );

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index OSM file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    let input_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/osm")
        .join(region);
    let input_file = input_dir.join(format!("{}-latest.osm.pbf", region));

    let mut osm_reader =
        lib_geo::osm_reader::make_osm_reader(&input_file).context(OsmPbfReaderSnafu)?;

    let admins_geofinder: AdminGeoFinder = client
        .list_documents()
        .await
        .map_err(|err| ModelError::DocumentRetrievalError { source: err.into() })
        .context(ListDocumentSnafu)?
        .try_collect()
        .await
        .map_err(|err| ModelError::DocumentRetrievalError { source: err.into() })
        .context(ListDocumentSnafu)?;

    // Read the street configuration from the osm2mimir configuration / testing mode.
    let config_dir: PathBuf = PathBuf::from(CONFIG_PATH);

    let mut config: osm_importer::Settings = exporter_config::config_from(
        &config_dir,
        &["osm2mimir", "elasticsearch"],
        "testing",
        None,
        vec![],
    )
    .context(ConfigSnafu)?
    .try_into()
    .context(ConfigInvalidSnafu)?;
    config.container_poi.dataset = dataset.to_string();
    config.container_street.dataset = dataset.to_string();

    let streets: Vec<Street> = lib_geo::osm_reader::street::streets(
        &mut osm_reader,
        &admins_geofinder,
        &config.streets.exclusions,
        #[cfg(feature = "db-storage")]
        None,
    )
    .context(StreetOsmExtractionSnafu)?
    .into_iter()
    .map(|street| street.set_weight_from_admins())
    .collect();

    let _ = client
        .generate_index(&config.container_street, futures::stream::iter(streets))
        .await
        .context(PoiIndexCreationSnafu)?;

    Ok(Status::Done)
}
