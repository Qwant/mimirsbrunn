use clap::Parser;
use elastic_client::model::query::Query;
use elastic_client::remote::{connection_test_pool, Remote};
use elastic_query_builder::coord::Coord;
use elastic_query_builder::doc_type::root_doctype;
use elastic_query_builder::dsl::{build_query, QueryType};
use elastic_query_builder::filters::Filters;
use elastic_query_builder::settings::QuerySettings;
use places::addr::Addr;
use places::admin::Admin;
use places::ContainerDocument;
use serde_helpers::DEFAULT_LIMIT_RESULT_ES;

#[derive(Debug, Parser)]
#[clap(name = "query", about = "Querying Bragi from the commandline")]
struct Opt {
    /// Activate debug mode
    // short and long flags (-d, --debug) will be deduced from the field's name
    #[clap(short, long)]
    debug: bool,

    /// latitude
    #[clap(long = "lat")]
    latitude: Option<f32>,

    /// longitude
    #[clap(long = "lon")]
    longitude: Option<f32>,

    /// Search String
    q: String,
}

#[tokio::main]
async fn main() {
    let opt = Opt::parse();

    let client = connection_test_pool()
        .conn(Default::default())
        .await
        .expect("Elasticsearch Connection Established");

    let filters = match opt.latitude {
        Some(latitude) => {
            let longitude = opt.longitude.expect("longitude");
            Filters {
                coord: Some(Coord::new(longitude, latitude)),
                ..Default::default()
            }
        }
        None => Filters::default(),
    };

    let settings = QuerySettings::default();

    let dsl = build_query(
        &client.config.index_root,
        &opt.q,
        &filters,
        "fr",
        &settings,
        QueryType::PREFIX,
        Option::None,
        false,
    );

    println!("{}", dsl);

    let es_indices_to_search = vec![
        root_doctype(&client.config.index_root, Admin::static_doc_type()),
        root_doctype(&client.config.index_root, Addr::static_doc_type()),
    ];

    client
        .search_documents(
            es_indices_to_search,
            Query::QueryDSL(dsl),
            DEFAULT_LIMIT_RESULT_ES,
            None,
        )
        .await
        .unwrap()
        .iter()
        .enumerate()
        .for_each(|(i, v): (_, &serde_json::Value)| {
            println!("{}: {} | {} | {}", i, v["id"], v["name"], v["label"]);
        });
}
