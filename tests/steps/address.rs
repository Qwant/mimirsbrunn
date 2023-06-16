use async_trait::async_trait;
use cucumber::given;
use elastic_client::remote::connection_test_pool;
use snafu::ResultExt;

use crate::error::{self, Error};
use crate::state::{GlobalState, State, Step, StepStatus};
use crate::steps::admin::IndexCosmogony;
use crate::steps::download::download_bano;
use elastic_client::remote::Remote;
use elastic_client::ElasticsearchStorageConfig;
use test_harness::bano;

// Index Bano

// The first parameter is the region, it must match a directory name in the fixtures.
// The second parameter is optional, and it is the dataset. If no dataset is give,
// then the dataset is set to be the same as the region.
#[given(regex = r"bano file has been indexed for (\S+) as (\S+)$")]
async fn index_bano(state: &mut GlobalState, region: String, dataset: String) {
    state
        .execute_once(IndexBano { region, dataset })
        .await
        .expect("failed to index Bano file");
}

#[given(regex = r"bano file has been indexed for (\S+)$")]
async fn index_bano_default_dataset(state: &mut GlobalState, region: String) {
    let dataset = region.clone();
    index_bano(state, region, dataset).await;
}

/// Index a bano file for given region into ES.
///
/// This will require to import admins first.
#[derive(Debug, PartialEq)]
pub struct IndexBano {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexBano {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let Self { region, dataset } = self;

        let client = connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Could not establish connection to Elasticsearch");

        state
            .status_of(&IndexCosmogony {
                region: region.to_string(),
                dataset: dataset.to_string(),
            })
            .expect("You must index admins before indexing addresses");

        bano::index_addresses(&client, region, dataset, false)
            .await
            .map(|status| status.into())
            .context(error::IndexBanoSnafu)
    }
}

// This step is a condensed format for download + index

#[given(regex = r"addresses \(bano\) have been indexed for (.+) into (\S+) as (\S+)$")]
async fn addresses_available(
    state: &mut GlobalState,
    departments: String,
    region: String,
    dataset: String,
) {
    download_bano(state, departments, region.clone()).await;
    index_bano(state, region, dataset).await;
}

#[given(regex = r"addresses \(bano\) have been indexed for (.+) into (\S+)$")]
async fn addresses_available_default_dataset(
    state: &mut GlobalState,
    departments: String,
    region: String,
) {
    let dataset = region.clone();
    download_bano(state, departments, region.clone()).await;
    index_bano(state, region, dataset).await;
}
