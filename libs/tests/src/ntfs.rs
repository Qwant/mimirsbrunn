use snafu::{ResultExt, Snafu};

use std::path::PathBuf;

use common::document::ContainerDocument;
use mimir::{
    adapters::secondary::elasticsearch::ElasticsearchStorage,
    domain::{
        model::configuration::root_doctype_dataset,
        ports::secondary::storage::{Error as StorageError, Storage},
    },
};
use places::stop::Stop;

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
    let container = root_doctype_dataset(Stop::static_doc_type(), dataset);

    let index = client
        .find_container(container)
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index BANO file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    // Load file
    let base_path = env!("CARGO_MANIFEST_DIR");
    let config_dir: PathBuf = [base_path, "..", "..", "config"].iter().collect();
    let input_dir: PathBuf = [base_path, "..", "..", "tests", "fixtures", "ntfs", region]
        .iter()
        .collect();

    let mut settings: mimirsbrunn::settings::ntfs2mimir::Settings = common::config::config_from(
        &config_dir,
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

    mimirsbrunn::stops::index_ntfs(&input_dir, &settings, client)
        .await
        .expect("error while indexing Ntfs");

    Ok(Status::Done)
}
