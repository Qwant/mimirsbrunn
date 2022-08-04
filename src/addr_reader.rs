use async_compression::tokio::bufread::GzipDecoder;
use futures::{
    future,
    stream::{Stream, StreamExt, TryStreamExt},
};
use serde::de::DeserializeOwned;
use snafu::{futures::TryStreamExt as SnafuTryStreamExt, ResultExt, Snafu};
use std::{
    ffi::OsStr,
    marker::{Send, Sync},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs::{metadata, File},
    io::BufReader,
};
use tracing::{info_span, warn};
use tracing_futures::Instrument;

use crate::utils;
use places::addr::Addr;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("CSV Error: {}", source))]
    Csv { source: csv_async::Error },

    #[snafu(display("IO Error: {}", source))]
    InvalidIO { source: tokio::io::Error },

    #[snafu(display("Path does not exist: {}", source))]
    InvalidPath { source: tokio::io::Error },

    #[snafu(display("Invalid extention"))]
    InvalidExtention,
}

/// Size of the IO buffer over input CSV file
const CSV_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

/// Import the addresses found in path, using the given (Elastiscsearch) configuration and client.
/// The function `into_addr` is used to transform the item read in the file (Bano) into an actual
/// address.
pub async fn import_addresses_from_input_path<F, T>(
    path: PathBuf,
    has_headers: bool,
    into_addr: F,
) -> Result<impl Stream<Item = Addr>, Error>
where
    F: Fn(T) -> Result<Addr, crate::error::Error> + Send + Sync + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
{
    metadata(&path).await.context(InvalidPathSnafu)?;
    let into_addr = Arc::new(into_addr);

    let recs = records_from_path(&path, has_headers)
        .filter_map(|rec| future::ready(rec.map_err(|err| warn!("Invalid CSV: {}", err)).ok()));

    let stream = recs
        .chunks(1000)
        .map(move |addresses| {
            let into_addr = into_addr.clone();
            async move {
                tokio::spawn(async move {
                    let addresses = addresses
                        .into_iter()
                        .filter_map(|rec| {
                            into_addr(rec)
                                .map_err(|err| warn!("Invalid address has been ignored: {}", err))
                                .ok()
                        })
                        .filter(|addr| {
                            let empty_name = addr.street.name.is_empty();

                            if empty_name {
                                warn!(
                                    "Address {} has no street name and has been ignored.",
                                    addr.id
                                )
                            }

                            !empty_name
                        })
                        .collect::<Vec<_>>();

                    futures::stream::iter(addresses)
                })
                .await
                .expect("tokio task panicked")
            }
        })
        // This line will spawn at most num_cpus::get() tasks (running asynchronously)
        // and give them to tokio runtime,
        // so the real number of running threads is up to tokio runtime
        .buffered(num_cpus::get())
        .flatten();

    Ok(stream)
}

/// Same as records_from_file, but can take an entire directory as input
fn records_from_path<T>(
    path: &Path,
    has_headers: bool,
) -> impl Stream<Item = Result<T, Error>> + Send + 'static
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    utils::fs::walk_files_recursive(path)
        .context(InvalidIOSnafu)
        .try_filter_map(move |file| async move {
            Ok(match records_from_file(&file, has_headers).await {
                Ok(recs) => {
                    let csv_file = file.to_str();
                    let span = info_span!("Read CSV file", has_headers, csv_file);
                    Some(recs.instrument(span))
                }
                Err(err) => {
                    warn!("skipping invalid file {}: {}", file.display(), err);
                    None
                }
            })
        })
        .try_flatten()
}

async fn records_from_file<T>(
    file: &Path,
    has_headers: bool,
) -> Result<impl Stream<Item = Result<T, Error>> + Send + 'static, Error>
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    let file_read = BufReader::with_capacity(
        CSV_BUFFER_SIZE,
        File::open(file).await.context(InvalidIOSnafu)?,
    );

    let data_read = {
        if file.extension().and_then(OsStr::to_str) == Some("csv") {
            Box::new(file_read) as Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>
        } else if file.extension().and_then(OsStr::to_str) == Some("gz") {
            Box::new(GzipDecoder::new(file_read)) as _
        } else {
            return Err(Error::InvalidExtention);
        }
    };

    let records = csv_async::AsyncReaderBuilder::new()
        .has_headers(has_headers)
        .create_deserializer(data_read)
        .into_deserialize::<T>()
        .context(CsvSnafu);

    Ok(records)
}
