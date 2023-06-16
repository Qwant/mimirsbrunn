use async_trait::async_trait;
use cucumber::{then, when};
use elastic_client::remote::{connection_test_pool, Remote};

use crate::error::Error;
use crate::state::{GlobalState, State, Step, StepStatus};
use crate::steps::TEST_INDEX_ROOT;

use elastic_client::model::query::Query;
use elastic_client::ElasticsearchStorageConfig;
use elastic_query_builder::dsl::build_features_query;
use elastic_query_builder::indices::build_es_indices_to_search;

// get place
#[when(regex = r#"the user ask for id "(.+)" with poi_dataset "(.+)" and pt_dataset "(.+)"$"#)]
async fn get(state: &mut GlobalState, id: String, pt_dataset: String, poi_dataset: String) {
    perform_get(state, id, pt_dataset, poi_dataset).await;
}

#[when(regex = r#"the user ask for id "(.+)" with pt_dataset "(.+)"$"#)]
async fn get_no_poi(state: &mut GlobalState, id: String, pt_dataset: String) {
    perform_get(state, id, pt_dataset, "".to_string()).await;
}

async fn perform_get(state: &mut GlobalState, id: String, pt_dataset: String, poi_dataset: String) {
    let pt_datasets = pt_dataset
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect();

    let poi_datasets = poi_dataset
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect();

    let indexes = build_es_indices_to_search(
        TEST_INDEX_ROOT,
        &None,
        &Some(pt_datasets),
        &Some(poi_datasets),
    );

    state
        .execute(Get {
            id,
            indexes,
            results: Vec::new(),
        })
        .await
        .expect("failed to search");
}

/// Perform a search in current Elasticsearch DB.
#[derive(Debug)]
pub struct Get {
    id: String,           // Id we are looking for
    indexes: Vec<String>, // ES indexes in which we will search
    results: Vec<serde_json::Value>,
}

#[async_trait(?Send)]
impl Step for Get {
    async fn execute(&mut self, _state: &State) -> Result<StepStatus, Error> {
        let client = connection_test_pool()
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .expect("Could not establish connection to Elasticsearch");

        // Build ES query
        let dsl = build_features_query(&self.indexes, &self.id);

        // Fetch documents
        self.results = {
            client
                .get_documents_by_id(Query::QueryDSL(dsl), None)
                .await
                .unwrap()
        };

        Ok(StepStatus::Done)
    }
}

// Find document

#[then(regex = r#"he gets "(.+)" as the first result, with name "(.+)"$"#)]
async fn find_id(state: &mut GlobalState, id: String, name: String) {
    state
        .execute(HasDocument {
            id,
            name,
            max_rank: 1,
        })
        .await
        .expect("failed to find document");
}

/// Check if given document is in the output.
///
/// It assumes that a Search has already been performed before.
pub struct HasDocument {
    id: String,
    name: String,
    max_rank: usize,
}

#[async_trait(?Send)]
impl Step for HasDocument {
    async fn execute(&mut self, state: &State) -> Result<StepStatus, Error> {
        let (search, _) = state
            .steps_for::<Get>()
            .next_back()
            .expect("the user must perform a search before checking results");

        let (rank, object) = (search.results.iter().enumerate())
            .find(|(_, doc)| {
                println!("{} vs {}", self.id, doc["id"].as_str().unwrap());
                self.id == doc["id"].as_str().unwrap()
            })
            .expect("document was not found in search results");

        assert_eq!(object["name"], self.name);
        assert!(rank < self.max_rank);
        Ok(StepStatus::Done)
    }
}
