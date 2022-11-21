use clap::Parser;
use common::document::ContainerDocument;
use mimir::{
    adapters::{
        primary::{
            bragi::api::DEFAULT_LIMIT_RESULT_ES,
            common::{
                coord::Coord,
                dsl::{build_query, QueryType},
                filters::Filters,
                settings::QuerySettings,
            },
        },
        secondary::elasticsearch::remote::connection_test_pool,
    },
    domain::{
        model::{configuration::root_doctype, query::Query},
        ports::{primary::search_documents::SearchDocuments, secondary::remote::Remote},
    },
};
use places::{addr::Addr, admin::Admin};

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
