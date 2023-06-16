use async_trait::async_trait;
use cucumber::given;
use elastic_client::remote::{connection_test_pool, Remote};
use snafu::ResultExt;

use crate::error::{self, Error};
use crate::state::{GlobalState, State, Step, StepStatus};
use crate::steps::admin::IndexCosmogony;
use crate::steps::download::download_ntfs;
use elastic_client::ElasticsearchStorageConfig;
use test_harness::ntfs;

#[given(regex = r"ntfs file has been indexed for ([^\s]+) as ([^\s]+)$")]
async fn index_ntfs(state: &mut GlobalState, region: String, dataset: String) {
    state
        .execute(IndexNTFS { region, dataset })
        .await
        .expect("failed to index NTFS file");
}

#[given(regex = r"stops have been indexed for ([^\s]+) as ([^\s]+)$")]
async fn stops_available(state: &mut GlobalState, region: String, dataset: String) {
    download_ntfs(state, region.clone()).await;
    index_ntfs(state, region, dataset).await;
}

/// Index an NTFS file for a given region into Elasticsearch.
///
/// This will require to import admins first.
#[derive(Debug, PartialEq)]
pub struct IndexNTFS {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexNTFS {
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

        ntfs::index_stops(&client, region, dataset, false)
            .await
            .map(|status| status.into())
            .context(error::IndexNTFSSnafu)
    }
}
