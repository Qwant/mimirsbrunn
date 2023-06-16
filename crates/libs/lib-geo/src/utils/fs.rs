use futures::stream::{Stream, TryStreamExt};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::Error;
use tokio_stream::wrappers::ReadDirStream;

/// Walk over all files of an input path. If it is a directory all files from
/// this directory or subdirectory are yielded, if it is a file, it will be the
/// only yielded value.
pub fn walk_files_recursive(
    path: &Path,
) -> impl Stream<Item = Result<PathBuf, Error>> + Send + Sync + 'static {
    let heap = vec![path.to_path_buf()];

    futures::stream::try_unfold(heap, |mut heap| async move {
        while let Some(curr) = heap.pop() {
            if curr.is_file() {
                return Ok(Some((curr, heap)));
            }

            let sub_dirs: Vec<_> = fs::read_dir(&curr)
                .await
                .map(ReadDirStream::new)?
                .try_collect()
                .await?;

            heap.extend(sub_dirs.into_iter().map(|dir| dir.path()))
        }

        Ok(None)
    })
}
