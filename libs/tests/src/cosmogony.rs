use snafu::{ResultExt, Snafu};
use std::path::PathBuf;

use super::utils::{create_dir_if_not_exists, file_exists};
use common::document::ContainerDocument;
use mimir::{
    adapters::secondary::elasticsearch::ElasticsearchStorage,
    domain::{
        model::configuration::{root_doctype_dataset, ContainerConfig, ContainerVisibility},
        ports::secondary::storage::{Error as StorageError, Storage},
    },
};
use places::admin::Admin;

use super::utils;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Could not Create Directory: {}", source))]
    CreateDir { source: utils::Error },
    #[snafu(display("Invalid IO: {} ({})", source, details))]
    InvalidIO {
        details: String,
        source: std::io::Error,
    },
    #[snafu(display("Invalid JSON: {} ({})", source, details))]
    Json {
        details: String,
        source: serde_json::Error,
    },
    #[snafu(display("NTFS Dataset not found"))]
    NtfsDatasetNotFound,
    #[snafu(display("Environment Variable Error: {} ({})", source, details))]
    EnvironmentVariable {
        details: String,
        source: std::env::VarError,
    },
    #[snafu(display("Indexing Error: {}", details))]
    Indexing { details: String },

    #[snafu(display("Container Search Error: {}", source))]
    ContainerSearch { source: StorageError },
}

pub enum Status {
    Skipped,
    Done,
}

pub async fn generate(region: &str, regenerate_if_already_exists: bool) -> Result<Status, Error> {
    // Build
    let base_path = env!("CARGO_MANIFEST_DIR");

    let input_dir: PathBuf = [base_path, "..", "..", "tests", "fixtures", "osm", region]
        .iter()
        .collect();
    let input_file = input_dir.join(format!("{}-latest.osm.pbf", region));

    let output_dir: PathBuf = [
        base_path,
        "..",
        "..",
        "tests",
        "fixtures",
        "cosmogony",
        region,
    ]
    .iter()
    .collect();
    let output_file = output_dir.join(format!("{}.jsonl.gz", region));

    create_dir_if_not_exists(&output_dir)
        .await
        .context(CreateDirSnafu)?;

    // If the output already exists, and we don't need to regenerate it, skip this step.
    if file_exists(&output_file).await && !regenerate_if_already_exists {
        return Ok(Status::Skipped);
    }

    // FIXME It would be nice not to resort to an env variable, and to use cosmogony as a library.
    let cosmogony_path = std::env::var("COSMOGONY_EXE").context(EnvironmentVariableSnafu {
        details: "Could not get COSMOGONY_EXE environment variable".to_string(),
    })?;

    // TODO: check command status ?
    tokio::process::Command::new(&cosmogony_path)
        .args(["--country-code", "FR"])
        .arg("--input")
        .arg(&input_file)
        .arg("--output")
        .arg(&output_file)
        .spawn()
        .expect("failed to spawn cosmogony")
        .wait()
        .await
        .context(InvalidIOSnafu {
            details: format!(
                "failed to generate cosmogony with input {} and output {}",
                input_file.display(),
                output_file.display()
            ),
        })?;

    Ok(Status::Done)
}

pub async fn index_admins(
    client: &ElasticsearchStorage,
    region: &str,
    dataset: &str,
    reindex_if_already_exists: bool,
    french_id_retrocompatibility: bool,
) -> Result<Status, Error> {
    // Check if the admin index already exists
    let container =
        root_doctype_dataset(&client.config.index_root, Admin::static_doc_type(), dataset);

    let index = client
        .find_container(container.clone())
        .await
        .context(ContainerSearchSnafu)?;

    // If the previous step has been skipped, then we don't need to index the
    // cosmogony file.
    if index.is_some() && !reindex_if_already_exists {
        return Ok(Status::Skipped);
    }

    let base_path = env!("CARGO_MANIFEST_DIR");
    let input_dir: PathBuf = [
        base_path,
        "..",
        "..",
        "tests",
        "fixtures",
        "cosmogony",
        region,
    ]
    .iter()
    .collect();
    let input_file = input_dir.join(format!("{}.jsonl.gz", region));

    mimirsbrunn::admin::index_cosmogony(
        &input_file,
        vec!["fr".to_string()],
        &ContainerConfig {
            name: Admin::static_doc_type().to_string(),
            dataset: dataset.to_string(),
            visibility: ContainerVisibility::Public,
            number_of_shards: 1,
            number_of_replicas: 0,
        },
        french_id_retrocompatibility,
        client,
    )
    .await
    .map_err(|err| Error::Indexing {
        details: format!("could not index cosmogony: {}", err,),
    })?;

    Ok(Status::Done)
}
