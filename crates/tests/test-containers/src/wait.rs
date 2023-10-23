use std::time::Duration;

/// A condition to meet before a container is considered ready
pub enum ReadyCondition {
    /// Waits for a stdout message
    _Stdout(String),
    /// Wait for a message on an http endpoint
    HttpPull {
        url: String,
        expect: String,
        interval: Duration,
    },
    /// Wait for docker healthcheck (see: https://docs.docker.com/engine/reference/builder/#healthcheck)
    _Healthy,
}
