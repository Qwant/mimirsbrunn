use snafu::{ResultExt, Snafu};

use std::path::PathBuf;

use elastic_client::storage::Error as StorageError;
use elastic_client::ElasticsearchStorage;
use elastic_query_builder::doc_type::root_doctype_dataset;
use exporter_config::CONFIG_PATH;
use places::stop::Stop;
use places::ContainerDocument;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },
}

pub enum Status {
    Skipped,
    Done,
}

pub async fn index_stops(
    client: &ElasticsearchStorage,
    region: &str,
    dataset: &str,
    reindex_if_already_exists: bool,
) -> Result<Status, Error> {
    let container =
        root_doctype_dataset(&client.config.index_root, Stop::static_doc_type(), dataset);

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index BANO file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    // Load file
    let input_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/ntfs/")
        .join(region);

    let mut settings: ntfs_importer::Settings = exporter_config::config_from(
        PathBuf::from(CONFIG_PATH).as_path(),
        &["ntfs2mimir", "elasticsearch"],
        "testing",
        None,
        vec![],
    )
    .expect("could not load ntfs2mimir configuration")
    .try_into()
    .expect("invalid ntfs2mimir configuration");

    // Use dataset set by test instead of default config
    settings.container.dataset = dataset.to_string();

    ntfs_importer::stops::index_ntfs(&input_dir, &settings, client)
        .await
        .expect("error while indexing Ntfs");

    Ok(Status::Done)
}
