use cucumber::WorldInit;
use mimir::utils::docker;
use state::GlobalState;

mod error;
mod state;
mod steps;

#[tokio::main]
async fn main() {
    docker::initialize()
        .await
        .expect("elasticsearch docker initialization");

    GlobalState::cucumber()
        .max_concurrent_scenarios(1)
        .filter_run("./features", |_, _, sc| {
            sc.tags.iter().any(|tag| tag == "unittest")
        })
        .await;
}
