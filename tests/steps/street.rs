use async_trait::async_trait;
use cucumber::given;
use mimir::{
    adapters::secondary::elasticsearch::remote::connection_test_pool,
    domain::ports::secondary::remote::Remote,
};
use snafu::ResultExt;

use crate::{
    error::{self, Error},
    state::{GlobalState, State, Step, StepStatus},
    steps::{admin::IndexCosmogony, download::download_osm},
};
use mimir::adapters::secondary::elasticsearch::ElasticsearchStorageConfig;
use tests::osm;

#[given(regex = r"streets have been indexed for ([^\s]+) as ([^\s]+)$")]
async fn index_streets(state: &mut GlobalState, region: String, dataset: String) {
    download_osm(state, region.clone()).await;

    state
        .execute_once(IndexStreets { region, dataset })
        .await
        .expect("failed to index OSM file for streets");
}

/// Index an osm file for a given region into Elasticsearch, extracting streets
///
/// This will require to import admins first.
#[derive(PartialEq)]
pub struct IndexStreets {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexStreets {
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
            .expect("You must index admins before indexing stops");

        osm::index_streets(&client, region, dataset, false)
            .await
            .map(|status| status.into())
            .context(error::IndexOsmSnafu)
    }
}
