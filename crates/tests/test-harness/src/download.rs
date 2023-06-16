use futures::{stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;
use tokio::fs;

use super::utils;

const GEOFABRIK_URL: &str = "https://download.geofabrik.de";
const BANO_URL: &str = "http://bano.openstreetmap.fr";
const OPENDATASOFT_URL: &str = "https://navitia.opendatasoft.com";

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Could not Download: {}", source))]
    Download { source: utils::Error },
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
}

pub enum Status {
    Skipped,
    Done,
}

pub async fn osm(region: &str) -> Result<Status, Error> {
    // Try to see if there is already a file with the expected name in tests/data/osm, in which
    // case we skip the actual download, to save time.
    let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/osm/")
        .join(region);

    let file_path = dir_path.join(format!("{}-latest.osm.pbf", region));

    if utils::file_exists(&file_path).await {
        return Ok(Status::Skipped);
    }

    // No file in 'cache', so we download it
    utils::create_dir_if_not_exists_rec(&dir_path)
        .await
        .context(CreateDirSnafu)?;
    let url = format!("{}/europe/france/{}-latest.osm.pbf", GEOFABRIK_URL, region);
    utils::download_to_file(&file_path, &url)
        .await
        .context(DownloadSnafu)?;

    Ok(Status::Done)
}

pub async fn bano<D: AsRef<str>>(region: &str, departments: &[D]) -> Result<Status, Error> {
    let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/bano/")
        .join(region);

    utils::create_dir_if_not_exists_rec(&dir_path)
        .await
        .context(CreateDirSnafu)?;

    // this is the path for the concatenated departments
    let file_path = &dir_path.join(format!("{}.csv", region));

    if utils::file_exists(file_path).await {
        return Ok(Status::Skipped);
    }

    stream::iter(departments.iter().map(Ok))
        .try_for_each(|department| async move {
            let url = format!("{}/data/bano-{:02}.csv", BANO_URL, department.as_ref());
            utils::download_to_file(file_path, &url)
                .await
                .context(DownloadSnafu)
        })
        .await?;

    Ok(Status::Done)
}

pub async fn ntfs(region: &str) -> Result<Status, Error> {
    let dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/ntfs/")
        .join(region);

    utils::create_dir_if_not_exists_rec(&dir_path)
        .await
        .context(CreateDirSnafu)?;

    let zip_file_path = dir_path.join(format!("{}.zip", region));
    let json_file_path = dir_path.join(format!("{}.json", region));

    if utils::file_exists(&zip_file_path).await {
        return Ok(Status::Skipped);
    }

    let url = format!(
        "{}/explore/dataset/{}/download/?format=json",
        OPENDATASOFT_URL, region
    );

    utils::download_to_file(&json_file_path, &url)
        .await
        .context(DownloadSnafu)?;

    let datasets = fs::read_to_string(&json_file_path)
        .await
        .context(InvalidIOSnafu {
            details: format!(
                "Could not read content of NTFS first download {}",
                &json_file_path.display()
            ),
        })?;
    let datasets: Vec<NTFSDataset> = serde_json::from_str(&datasets).context(JsonSnafu {
        details: "Could not deserialize NTFS datasets",
    })?;
    let url = datasets
        .iter()
        .find_map(|dataset| {
            if dataset.fields.format == "NTFS" {
                let url = format!(
                    "{}/api/v2/catalog/datasets/{}/files/{}",
                    OPENDATASOFT_URL, region, dataset.fields.download.id
                );
                Some(url)
            } else {
                None
            }
        })
        .ok_or(Error::NtfsDatasetNotFound)?;

    fs::remove_file(json_file_path.as_path())
        .await
        .context(InvalidIOSnafu {
            details: format!("Could not remove {}", json_file_path.display()),
        })?;

    utils::download_to_file(&zip_file_path, &url)
        .await
        .context(DownloadSnafu)?;

    let _res = tokio::task::spawn_blocking(move || {
        // Straight from example in zip crate.
        let file = std::fs::File::open(&zip_file_path).unwrap();

        let mut archive = zip::ZipArchive::new(file).unwrap();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let outpath = match file.enclosed_name() {
                Some(path) => dir_path.join(path),
                None => continue,
            };

            {
                let comment = file.comment();
                if !comment.is_empty() {
                    println!("File {} comment: {}", i, comment);
                }
            }

            if file.name().ends_with('/') {
                println!("File {} extracted to \"{}\"", i, outpath.display());
                std::fs::create_dir_all(&outpath).unwrap();
            } else {
                println!(
                    "File {} extracted to \"{}\" ({} bytes)",
                    i,
                    outpath.display(),
                    file.size()
                );
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p).unwrap();
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).unwrap();
                std::io::copy(&mut file, &mut outfile).unwrap();
            }

            // Get and Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                        .unwrap();
                }
            }
        }
        0
    })
    .await
    .unwrap();

    Ok(Status::Done)
}

#[derive(Debug, Serialize, Deserialize)]
struct NTFSDownload {
    format: String,
    filename: String,
    width: u32,
    id: String,
    height: u32,
    thumbnail: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct NTFSFields {
    license_link: String,
    update_date: String,
    description: String,
    license: String,
    format: String,
    validity_end_date: String,
    validity_start_date: String,
    download: NTFSDownload,
    id: String,
    size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct NTFSDataset {
    datasetid: String,
    recordid: String,
    fields: NTFSFields,
    record_timestamp: String,
}
