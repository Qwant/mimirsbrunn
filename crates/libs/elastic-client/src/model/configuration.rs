use serde::{Deserialize, Serialize};

use crate::errors::{ElasticClientError, Result};

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

pub fn split_index_name(name: &str) -> Result<(&str, &str, &str)> {
    let parts: Vec<_> = name.split('_').collect();
    if parts.len() < 3 {
        Err(ElasticClientError::InvalidIndexName(name.to_string()))
    } else {
        Ok((parts[0], parts[1], parts[2]))
    }
}

#[cfg(test)]
mod test {
    use speculoos::prelude::*;

    use crate::model::configuration::split_index_name;

    #[test]
    fn should_split_index_name() -> anyhow::Result<()> {
        let (one, two, three) = split_index_name("munin_admin_fr_20211104_152535_346903898")?;
        assert_that!(one).is_equal_to("munin");
        assert_that!(two).is_equal_to("admin");
        assert_that!(three).is_equal_to("fr");
        Ok(())
    }
}
