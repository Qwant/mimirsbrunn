use cucumber::{async_trait, criteria::feature, futures::FutureExt, Context, Cucumber, World};
use elasticsearch::http::transport::SingleNodeConnectionPool;
use mimir::adapters::primary::common::settings::QuerySettings;
use mimir::adapters::secondary::elastic_client::remote::connection_test_pool;
use mimir::adapters::secondary::elasticsearch::{ES_DEFAULT_TIMEOUT, ES_DEFAULT_VERSION_REQ};
use elastic_client::remote::Remote;
use mimir::utils::docker;
use std::convert::Infallible;

mod steps;

pub struct MyWorld {
    query_settings: QuerySettings,
    search_result: Vec<serde_json::Value>,
    processing_step: Option<steps::admin::ProcessingStep>,
}

#[async_trait(?Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        let query_settings = QuerySettings::default();
        Ok(Self {
            query_settings,
            search_result: Vec::new(),
            processing_step: None,
        })
    }
}

#[tokio::main]
async fn main() {
    let _guard = test_containers::initialize()
        .await
        .expect("elasticsearch docker initialization");
    let pool = connection_test_pool().await.unwrap();

    Cucumber::<MyWorld>::new()
        // Specifies where our feature files exist
        .features(&["./features/stop"])
        // Adds the implementation of our steps to the runner
        .steps(steps::stop::steps())
        // Add some global context for all the tests, like databases.
        .context(Context::new().add(pool))
        // Add some lifecycle functions to manage our database nightmare
        .before(feature("Example feature"), |ctx| {
            // FIXME What should be done with these before and after?
            // Should we create the client here?
            let pool = ctx.get::<SingleNodeConnectionPool>().unwrap().clone();
            async move {
                let _client = pool
                    .conn(ES_DEFAULT_TIMEOUT, ES_DEFAULT_VERSION_REQ)
                    .await
                    .unwrap();
            }
            .boxed()
        })
        // Parses the command line arguments if passed
        .cli()
        // Runs the Cucumber tests and then exists
        .run_and_exit()
        .await
}
