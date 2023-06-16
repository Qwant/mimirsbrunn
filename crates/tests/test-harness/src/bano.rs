use futures::stream::StreamExt;
use snafu::{ResultExt, Snafu};

use std::path::PathBuf;
use std::sync::Arc;

use elastic_client::storage::Error as StorageError;
use elastic_client::ElasticsearchStorage;
use elastic_query_builder::doc_type::root_doctype_dataset;
use exporter_config::CONFIG_PATH;
use lib_geo::bano::Bano;
use places::addr::Addr;
use places::admin::Admin;
use places::ContainerDocument;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },

    #[snafu(display("Address Fetching Error: {}", source))]
    AddressFetch { source: lib_geo::addr_reader::Error },
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
    let container =
        root_doctype_dataset(&client.config.index_root, Addr::static_doc_type(), dataset);

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index BANO file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    let input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/bano/")
        .join(region)
        .join(format!("{}.csv", region));

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
    let mut config: bano_importer::Settings = exporter_config::config_from(
        PathBuf::from(CONFIG_PATH).as_path(),
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
        lib_geo::addr_reader::import_addresses_from_input_path(input_file, false, into_addr)
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
