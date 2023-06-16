use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Invalid Index Configuration: {}", details))]
    InvalidConfiguration { details: String },

    #[snafu(display("Serialization Error: {}", details))]
    Serialization { details: String },

    #[snafu(display("Invalid Name: {}", details))]
    InvalidName { details: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContainerConfig {
    pub name: String,
    pub dataset: String,
    pub visibility: ContainerVisibility,
    pub number_of_shards: u64,
    pub number_of_replicas: u64,
    pub min_expected_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalModeWeight {
    pub id: String,
    pub weight: f32,
}

// Given an index name in the form {}_{}_{}_{}, we extract the 2nd and 3rd
// pieces which are supposed to be respectively the doc_type and the dataset.
pub fn split_index_name(name: &str) -> Result<(&str, &str, &str), Error> {
    lazy_static! {
        static ref SPLIT_INDEX_NAME: Regex = Regex::new(r"([^_]+)_([^_]+)_([^_]+)_*").unwrap();
    }

    if let Some(caps) = SPLIT_INDEX_NAME.captures(name) {
        let root = caps
            .get(1)
            .ok_or_else(|| Error::InvalidName {
                details: name.to_string(),
            })?
            .as_str();

        let doc_type = caps
            .get(2)
            .ok_or_else(|| Error::InvalidName {
                details: name.to_string(),
            })?
            .as_str();

        let dataset = caps
            .get(3)
            .ok_or_else(|| Error::InvalidName {
                details: name.to_string(),
            })?
            .as_str();

        Ok((root, doc_type, dataset))
    } else {
        Err(Error::InvalidName {
            details: format!("Could not analyze index name: {}", name),
        })
    }
}
