use snafu::{ResultExt, Snafu};
use std::{fs::File, io::BufReader, path::Path};

pub mod admin;
pub mod osm_store;
pub mod osm_utils;
pub mod poi;
pub mod street;

pub type OsmPbfReader = osmpbfreader::OsmPbfReader<BufReader<File>>;

/// Size of the IO buffer over input PBF file
const PBF_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("IO Error: {}", source))]
    IO { source: std::io::Error },
}

pub fn make_osm_reader(path: &Path) -> Result<OsmPbfReader, Error> {
    let file = File::open(path).context(IOSnafu)?;

    Ok(osmpbfreader::OsmPbfReader::new(BufReader::with_capacity(
        PBF_BUFFER_SIZE,
        file,
    )))
}
