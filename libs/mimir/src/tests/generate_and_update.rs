use futures::{stream, TryStreamExt};
use serial_test::serial;

use crate::{
    adapters::secondary::elasticsearch::{remote, ElasticsearchStorageConfig},
    domain::{
        model::{
            configuration::{ContainerConfig, ContainerVisibility},
            update::UpdateOperation,
        },
        ports::{
            primary::{generate_index::GenerateIndex, list_documents::ListDocuments},
            secondary::remote::Remote,
        },
    },
    utils::docker,
};
use places::poi::Poi;

fn sample_poi() -> Poi {
    Poi {
        id: "osm:poi:1".to_string(),
        name: "eiffel tower".to_string(),
        zip_codes: vec!["75007".to_string()],
        ..Poi::default()
    }
}

async fn generate_and_update_poi(id: &str, updates: Vec<UpdateOperation>) -> Vec<Poi> {
    docker::initialize()
        .await
        .expect("elasticsearch docker initialization failed");

    let client = remote::connection_test_pool()
        .conn(ElasticsearchStorageConfig::default_testing())
        .await
        .expect("could not connect to Elasticsearch");

    let container_config = ContainerConfig {
        name: "poi".to_string(),
        dataset: "test".to_string(),
        visibility: ContainerVisibility::Public,
        number_of_shards: 1,
        number_of_replicas: 0,
        min_expected_count: 1,
    };

    client
        .init_container(&container_config)
        .await
        .unwrap()
        .insert_documents(stream::iter([sample_poi()]))
        .await
        .unwrap()
        .update_documents(stream::iter([(id.to_string(), updates)]))
        .await
        .unwrap()
        .publish()
        .await
        .unwrap();

    client
        .list_documents()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn should_update_poi() {
    let documents = generate_and_update_poi(
        "osm:poi:1",
        vec![
            UpdateOperation::Set {
                ident: "name".to_string(),
                value: "tour eiffel".to_string(),
            },
            UpdateOperation::Set {
                ident: "properties.image".to_string(),
                value: "<URL>".to_string(),
            },
        ],
    )
    .await;

    let result_poi = documents.into_iter().next().unwrap();

    // Check that result_poi (fetched from the index) fields are updated
    assert_eq!(result_poi.name, "tour eiffel");
    assert_eq!(result_poi.properties["image"], "<URL>");

    // This should be untouched
    assert_eq!(result_poi.zip_codes, ["75007".to_string()]);
}

#[tokio::test]
#[should_panic]
#[serial]
async fn should_fail_updating_wrong_poi() {
    generate_and_update_poi(
        "this_is_not_a_poi",
        vec![
            UpdateOperation::Set {
                ident: "name".to_string(),
                value: "tour eiffel".to_string(),
            },
            UpdateOperation::Set {
                ident: "properties.image".to_string(),
                value: "<URL>".to_string(),
            },
        ],
    )
    .await;
}
