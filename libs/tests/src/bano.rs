use futures::stream::StreamExt;
use mimir::domain::ports::primary::generate_index::GenerateIndex;
use snafu::{ResultExt, Snafu};

use std::{path::PathBuf, sync::Arc};

use common::document::ContainerDocument;
use mimir::{
    adapters::secondary::elasticsearch::ElasticsearchStorage,
    domain::{
        model::configuration::root_doctype_dataset,
        ports::{
            primary::list_documents::ListDocuments,
            secondary::storage::{Error as StorageError, Storage},
        },
    },
};
use mimirsbrunn::bano::Bano;
use places::{addr::Addr, admin::Admin};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },

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
        let admins: Vec<Admin> = client
            .list_documents()
            .await
            .expect("could not query for admins")
            .map(|admin| admin.expect("could not parse admin"))
            .collect()
            .await;

        let admins_by_insee = admins
            .iter()
            .cloned()
            .filter(|addr| !addr.insee.is_empty())
            .map(|addr| (addr.insee.clone(), Arc::new(addr)))
            .collect();

        let admins_geofinder = admins.into_iter().collect();
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
    .try_deserialize()
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
