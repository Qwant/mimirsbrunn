use std::path::PathBuf;

use snafu::{ResultExt, Snafu};

use common::document::ContainerDocument;
use mimir::adapters::secondary::elasticsearch::ElasticsearchStorage;
use mimir::domain::model::configuration::root_doctype_dataset;
use mimir::domain::ports::primary::generate_index::GenerateIndex;
use mimir::domain::ports::secondary::storage::{Error as StorageError, Storage};
use mimirsbrunn::admin_geofinder::AdminGeoFinder;
use mimirsbrunn::bano::Bano;
use mimirsbrunn::settings::admin_settings::AdminSettings;
use places::addr::Addr;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },

    #[snafu(display("Load Admins Error {}", source))]
    LoadAdmins { source: mimirsbrunn::admin::Error },

    #[snafu(display("Address Fetching Error: {}", source))]
    AddressFetch {
        source: mimirsbrunn::addr_reader::Error,
    },
}

pub enum Status {
    Skipped,
    Done,
}

pub async fn index_addresses(
    client: &ElasticsearchStorage,
    region: &str,
    dataset: &str,
    reindex_if_already_exists: bool,
) -> Result<Status, Error> {
    // Check if the address index already exists
    let container = root_doctype_dataset(Addr::static_doc_type(), dataset);

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index BANO file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    let base_path = env!("CARGO_MANIFEST_DIR");
    let config_dir: PathBuf = [base_path, "..", "..", "config"].iter().collect();
    let input_dir: PathBuf = [base_path, "..", "..", "tests", "fixtures", "bano", region]
        .iter()
        .collect();
    let input_file = input_dir.join(format!("{}.csv", region));

    // TODO: there might be some factorisation to do with bano2mimir?
    let into_addr = {
        let admins_geofinder = AdminGeoFinder::build(&AdminSettings::Elasticsearch, client)
            .await
            .context(LoadAdminsSnafu)?;

        let admins_by_insee = admins_geofinder
            .iter()
            .filter(|admin| !admin.insee.is_empty())
            .map(|admin| (admin.insee.clone(), admin))
            .collect();

        move |b: Bano| b.into_addr(&admins_by_insee, &admins_geofinder)
    };

    // Load file
    let mut config: mimirsbrunn::settings::bano2mimir::Settings = common::config::config_from(
        &config_dir,
        &["bano2mimir", "elasticsearch"],
        "testing",
        None,
        vec![],
    )
    .expect("could not load bano2mimir configuration")
    .try_into()
    .expect("invalid bano2mimir configuration");
    config.container.dataset = dataset.to_string();

    let addresses =
        mimirsbrunn::addr_reader::import_addresses_from_input_path(input_file, false, into_addr)
            .await
            .context(AddressFetchSnafu)?;

    client
        .generate_index(&config.container, addresses)
        .await
        .map_err(|err| Error::Indexing {
            details: format!("could not index bano: {}", err,),
        })?;

    Ok(Status::Done)
}
