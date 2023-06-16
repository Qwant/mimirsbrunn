use std::fmt;
use std::time::Duration;

use futures::Future;
use tracing::warn;

/// Perform an async operation with exponential backoff. If all retries fail,
/// the last error is outputed.
///
///  - `task`: the async task to perform
///  - `retries`: number of tries to perform after the first failure
///  - `backoff`: waiting time after the first failure
pub async fn with_backoff<T, E, F>(
    mut task: impl FnMut() -> F,
    mut retries: u8,
    mut wait: Duration,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: fmt::Display,
{
    let mut res = task().await;

    while retries > 0 {
        if let Err(err) = res {
            warn!("retry in {:?} after error in task: {}", wait, err);
            tokio::time::sleep(wait).await;
            res = task().await;
            retries -= 1;
            wait *= 2;
        } else {
            break;
        }
    }

    res
}
