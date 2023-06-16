use criterion::{criterion_group, criterion_main, Criterion};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::File;

use elastic_client::model::query::Query;
use elastic_client::remote::{connection_test_pool, Remote};
use elastic_client::ElasticsearchStorageConfig;
use elastic_query_builder::dsl::{build_query, QueryType};
use elastic_query_builder::filters::Filters;
use elastic_query_builder::settings::QuerySettings;
use serde_helpers::DEFAULT_LIMIT_RESULT_ES;
use test_harness::{bano, cosmogony, download, ntfs, osm};

fn bench(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(6)
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    rt.block_on(async {
        test_containers::initialize()
            .await
            .expect("elasticsearch docker initialization");
        let config = ElasticsearchStorageConfig::default_testing();
        let client = connection_test_pool()
            .conn(config)
            .await
            .expect("could not establish connection with Elasticsearch");

        download::osm("ile-de-france").await.unwrap();
        download::bano("ile-de-france", &["75", "77", "78", "92", "93", "94", "95"])
            .await
            .unwrap();
        download::ntfs("fr-idf").await.unwrap();
        // false: don't force regenerate admins for 'ile-de-france'
        cosmogony::generate("ile-de-france", false).await.unwrap();
        // true: force reindex admins on bench dataset for 'ile-de-france'
        cosmogony::index_admins(&client, "ile-de-france", "bench", true, true)
            .await
            .unwrap();
        bano::index_addresses(&client, "ile-de-france", "bench", true)
            .await
            .unwrap();
        osm::index_pois(&client, "ile-de-france", "bench", true)
            .await
            .unwrap();
        osm::index_streets(&client, "ile-de-france", "bench", true)
            .await
            .unwrap();
        ntfs::index_stops(&client, "fr-idf", "bench", true)
            .await
            .unwrap();
    });

    let mut group = c.benchmark_group("searching");
    group.bench_function("searching addresses", |b| {
        b.iter(|| {
            rt.block_on(async move {
                let config = ElasticsearchStorageConfig::default_testing();
                let client = connection_test_pool()
                    .conn(config)
                    .await
                    .expect("could not establish connection with Elasticsearch");
                let filters = Filters::default();

                let settings = QuerySettings::default();

                let csv_path: PathBuf = [
                    env!("CARGO_MANIFEST_DIR"),
                    "tests",
                    "fixtures",
                    "geocoder",
                    "idf-addresses.csv",
                ]
                .iter()
                .collect();
                let reader = File::open(csv_path)
                    .await
                    .expect("geocoder addresses csv file");
                let csv_reader = csv_async::AsyncReaderBuilder::new()
                    .has_headers(false)
                    .create_deserializer(reader);
                let stream = csv_reader.into_deserialize::<Record>();
                stream
                    .for_each(|rec| {
                        let rec = rec.unwrap();
                        let client = client.clone();
                        let filters = filters.clone();
                        let dsl = build_query(
                            "munin",
                            &rec.query,
                            &filters,
                            "fr",
                            &settings,
                            QueryType::PREFIX,
                            None,
                            false,
                        );

                        async move {
                            let _values = client
                                .search_documents::<serde_json::Value>(
                                    vec!["munin".to_string()],
                                    Query::QueryDSL(dsl),
                                    DEFAULT_LIMIT_RESULT_ES,
                                    None,
                                )
                                .await
                                .unwrap();
                        }
                    })
                    .await;
            })
        });
    });
    group.finish();
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    pub query: String,
    pub lon: Option<String>,
    pub lat: Option<String>,
    pub limit: Option<String>,
    pub expected_housenumber: Option<String>,
    pub expected_street: Option<String>,
    pub expected_city: Option<String>,
    pub expected_postcode: Option<String>,
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench
}
criterion_main!(benches);
