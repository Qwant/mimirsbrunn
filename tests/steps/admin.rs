use async_trait::async_trait;
use cucumber::given;
use elastic_client::remote::{connection_test_pool, Remote};
use snafu::ResultExt;

use crate::error;
use crate::error::Error;
use crate::state::{GlobalState, State, Step, StepStatus};
use crate::steps::download::{download_osm, DownloadOsm};
use elastic_client::ElasticsearchStorageConfig;
use test_harness::cosmogony;

// Generate Cosmogony

#[given(regex = r"osm file has been processed by cosmogony for (\S+)$")]
async fn generate_cosmogony(state: &mut GlobalState, region: String) {
    state
        .execute_once(GenerateCosmogony(region))
        .await
        .expect("failed to generate cosmogony file");
}

#[derive(PartialEq)]
pub struct GenerateCosmogony(pub String);

#[async_trait(?Send)]
impl Step for GenerateCosmogony {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let Self(region) = self;

        state
            .status_of(&DownloadOsm(region.to_string()))
            .expect("can't generate cosmogony file without downloading from OSM first");

        cosmogony::generate(region, false)
            .await
            .map(|status| status.into())
            .context(error::GenerateCosmogonySnafu)
    }
}

// Index Cosmogony

#[given(regex = r"cosmogony file has been indexed for (\S+) as (\S+)$")]
async fn index_cosmogony(state: &mut GlobalState, region: String, dataset: String) {
    state
        .execute_once(IndexCosmogony { region, dataset })
        .await
        .expect("failed to index cosmogony file");
}

#[given(regex = r"cosmogony file has been indexed for (\S+)$")]
async fn index_cosmogony_default_dataset(state: &mut GlobalState, region: String) {
    let dataset = region.clone();
    index_cosmogony(state, region, dataset).await
}

/// Index a cosmogony file for given region into ES.
///
/// This assumes that a cosmogony file has already been generated before.
#[derive(Debug, PartialEq)]
pub struct IndexCosmogony {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexCosmogony {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let Self { region, dataset } = self;

        let client = connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Could not establish connection to Elasticsearch");

        state
            .status_of(&GenerateCosmogony(region.to_string()))
            .expect("can't generate cosmogony file without downloading from OSM first");

        cosmogony::index_admins(&client, region, dataset, false, true)
            .await
            .map(|status| status.into())
            .context(error::IndexCosmogonySnafu)
    }
}

// This step is a condensed format for download + generate + index

#[given(regex = r"admins have been indexed for (\S+) as (\S+)$")]
async fn admins_available(state: &mut GlobalState, region: String, dataset: String) {
    download_osm(state, region.clone()).await;
    generate_cosmogony(state, region.clone()).await;
    index_cosmogony(state, region, dataset).await;
}

#[given(regex = r"admins have been indexed for (\S+)$")]
async fn admins_available_default_dataset(state: &mut GlobalState, region: String) {
    let dataset = region.clone();
    admins_available(state, region, dataset).await;
}
