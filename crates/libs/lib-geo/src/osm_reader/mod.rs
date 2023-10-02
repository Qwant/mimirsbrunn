use crate::osm_reader::errors::OsmReaderError;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub mod admin;
pub mod osm_store;
pub mod osm_utils;
pub mod poi;
pub mod street;

pub mod errors;

#[cfg(feature = "db-storage")]
pub mod database;

pub type OsmPbfReader = osmpbfreader::OsmPbfReader<BufReader<File>>;

/// Size of the IO buffer over input PBF file
const PBF_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

pub fn make_osm_reader(path: &Path) -> Result<OsmPbfReader, OsmReaderError> {
    let file = File::open(path)?;

    Ok(osmpbfreader::OsmPbfReader::new(BufReader::with_capacity(
        PBF_BUFFER_SIZE,
        file,
    )))
}
