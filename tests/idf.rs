use cucumber::WorldInit;
use mimir::utils::docker;
use state::GlobalState;

mod error;
mod state;
mod steps;

#[tokio::main]
async fn main() {
    let _guard = test_containers::initialize_with_param(true)
        .await
        .expect("elasticsearch docker initialization");

    GlobalState::cucumber()
        .max_concurrent_scenarios(1)
        .run("./features/idf")
        .await;
}
