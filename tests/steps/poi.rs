use async_trait::async_trait;
use cucumber::given;
use elastic_client::remote::connection_test_pool;
use snafu::ResultExt;

use crate::error::{self, Error};
use crate::state::{GlobalState, State, Step, StepStatus};
use crate::steps::admin::IndexCosmogony;
use crate::steps::download::download_osm;
use elastic_client::remote::Remote;
use elastic_client::ElasticsearchStorageConfig;
use test_harness::osm;

// Index POIs

#[given(regex = r"pois have been indexed for (\S+) as (\S+)$")]
async fn pois_available(state: &mut GlobalState, region: String, dataset: String) {
    download_osm(state, region.clone()).await;

    state
        .execute_once(IndexPois { region, dataset })
        .await
        .expect("failed to index OSM file for pois");
}

#[given(regex = r"pois have been indexed for (\S+)$")]
async fn pois_available_default_dataset(state: &mut GlobalState, region: String) {
    let dataset = region.clone();
    pois_available(state, region, dataset).await;
}

/// Index an osm file for a given region into Elasticsearch, extracting pois
///
/// This will require to import admins first.
#[derive(PartialEq)]
pub struct IndexPois {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexPois {
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
            .expect("You must index admins before indexing pois");

        osm::index_pois(&client, region, dataset, false)
            .await
            .map(|status| status.into())
            .context(error::IndexOsmSnafu)
    }
}
