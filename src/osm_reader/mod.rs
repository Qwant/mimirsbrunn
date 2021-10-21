use snafu::{ResultExt, Snafu};
use std::fs::File;
use std::path::Path;

pub mod admin;
pub mod osm_store;
pub mod osm_utils;
pub mod poi;
pub mod street;

pub type OsmPbfReader = osmpbfreader::OsmPbfReader<File>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("IO Error: {}", source))]
    IO { source: std::io::Error },
}

pub fn make_osm_reader(path: &Path) -> Result<OsmPbfReader, Error> {
    Ok(osmpbfreader::OsmPbfReader::new(
        File::open(&path).context(IO)?,
    ))
}
