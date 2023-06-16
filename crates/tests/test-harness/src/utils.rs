use snafu::{ResultExt, Snafu};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Invalid Download URL: {} ({})", source, details))]
    InvalidUrl {
        details: String,
        source: url::ParseError,
    },
    #[snafu(display("Invalid IO: {} ({})", source, details))]
    InvalidIO {
        details: String,
        source: std::io::Error,
    },
    #[snafu(display("Download Error: {} ({})", source, details))]
    Download {
        details: String,
        source: reqwest::Error,
    },
}

pub async fn file_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

pub async fn create_dir_if_not_exists(path: &Path) -> Result<(), Error> {
    if !file_exists(path).await {
        fs::create_dir(path).await.context(InvalidIOSnafu {
            details: format!("could no create directory {}", path.display()),
        })?;
    }

    Ok(())
}

pub async fn create_dir_if_not_exists_rec(path: &Path) -> Result<(), Error> {
    let mut head = PathBuf::new();

    for fragment in path {
        head.push(fragment);
        create_dir_if_not_exists(&head).await?;
    }

    Ok(())
}

/// Downloads the file identified by the url and saves it to the given path.
/// If a file is already present, it will append to that file.
pub async fn download_to_file(path: &Path, url: &str) -> Result<(), Error> {
    let mut file = tokio::io::BufWriter::new({
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .await
            .context(InvalidIOSnafu {
                details: format!("could no create file for download {}", path.display()),
            })?
    });

    let mut resp = reqwest::get(url)
        .await
        .context(DownloadSnafu {
            details: format!("could not download url {}", url),
        })?
        .error_for_status()
        .context(DownloadSnafu {
            details: format!("download response error for {}", url),
        })?;

    while let Some(chunk) = resp.chunk().await.context(DownloadSnafu {
        details: format!("read chunk error during download of {}", url),
    })? {
        file.write_all(&chunk).await.context(InvalidIOSnafu {
            details: format!("write chunk error during download of {}", url),
        })?;
    }

    file.flush().await.context(InvalidIOSnafu {
        details: format!("flush error during download of {}", url),
    })?;

    Ok(())
}
