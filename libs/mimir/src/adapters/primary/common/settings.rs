use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::path::Path;
use tokio::{
    fs::File,
    io::{AsyncReadExt, BufReader},
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Tokio IO Error: {}", source))]
    InvalidFileOpen { source: tokio::io::Error },

    #[snafu(display("TOML Error: {}", source))]
    InvalidFileContent { source: toml::de::Error },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Types {
    pub address: f64,
    pub admin: f64,
    pub stop: f64,
    pub poi: f64,
    pub street: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct TypeQueryBoosts {
    pub global: f64,
    pub boosts: Types,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct StringQueryBoosts {
    pub name: f64,
    pub alt_name: f64,
    pub loc_name: f64,
    pub label: f64,
    pub label_prefix: f64,
    pub zip_codes: f64,
    pub house_number: f64,
    pub label_ngram_with_coord: f64,
    pub label_ngram: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct StringQuery {
    pub global: f64,
    pub boosts: StringQueryBoosts,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Decay {
    pub func: String,
    pub scale: f64,
    pub offset: f64,
    pub decay: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Proximity {
    pub weight: f64,
    pub weight_fuzzy: f64,
    pub decay: Decay,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct BuildWeight {
    pub admin: f64,
    pub factor: f64,
    pub missing: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Weights {
    pub radius_range: (f64, f64),
    pub max_radius: BuildWeight,
    pub min_radius_prefix: BuildWeight,
    pub min_radius_fuzzy: BuildWeight,
    #[serde(flatten)]
    pub types: Types,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImportanceQueryBoosts {
    pub proximity: Proximity,
    pub weights: Weights,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ReverseQuery {
    pub radius: u32, // search radius in meters
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QuerySettings {
    pub type_query: TypeQueryBoosts,
    pub string_query: StringQuery,
    pub importance_query: ImportanceQueryBoosts,
    pub reverse_query: ReverseQuery,
}

// This wrapper is used because the configuration file should
// have the query settings definition in an object under the key 'query'
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QuerySettingsWrapper {
    pub query: QuerySettings,
}

impl QuerySettings {
    pub fn new(settings: &str) -> Result<QuerySettings, Error> {
        let wrapper: QuerySettingsWrapper =
            toml::from_str(settings).context(InvalidFileContentSnafu)?;
        Ok(wrapper.query)
    }

    pub async fn new_from_file<P>(path: P) -> Result<QuerySettings, Error>
    where
        P: AsRef<Path>,
    {
        let mut settings_content = String::new();

        let mut settings_file = File::open(path)
            .await
            .map(BufReader::new)
            .context(InvalidFileOpenSnafu)?;

        settings_file
            .read_to_string(&mut settings_content)
            .await
            .context(InvalidFileOpenSnafu)?;

        QuerySettings::new(&settings_content)
    }
}

impl Default for QuerySettings {
    fn default() -> Self {
        let settings = include_str!("../../../../../../config/query/default.toml");
        QuerySettings::new(settings)
            .expect("could not create default query settings. Check config/query/default.toml")
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn should_get_default_query_settings() {
        let _settings = QuerySettings::default();
    }
}
