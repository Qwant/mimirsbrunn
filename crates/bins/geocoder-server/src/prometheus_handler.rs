#[cfg(feature = "metrics")]
use prometheus::{Encoder, TextEncoder};

#[cfg(feature = "metrics")]
lazy_static::lazy_static! {
    static ref PATH_TO_NAME: std::collections::HashMap<&'static str, &'static str> = {
        let mut map = std::collections::HashMap::new();
        map.insert("/api/v1/", "/");
        map.insert("/api/v1/metrics", "metrics");
        map.insert("/api/v1/status", "status");
        map.insert("/api/v1/reverse", "reverse");
        map.insert("/api/v1/autocomplete", "autocomplete");
        map.insert("/api/v1/autocomplete-explain", "autocomplete-explain");
        map
    };

    static ref FEATURES_ROUTE: &'static str = "/api/v1/features";
}

#[cfg(feature = "metrics")]
fn get_ressource_name(path: &str) -> String {
    // we can't use the ressource's name in the current actix version,
    // so we use an hardcoded associated table
    PATH_TO_NAME
        .get(path)
        .copied()
        .unwrap_or_else(|| {
            if path.starts_with("/api/v1/features") {
                &FEATURES_ROUTE
            } else {
                ""
            }
        })
        .to_string()
}

#[cfg(feature = "metrics")]
lazy_static::lazy_static! {
    static ref HTTP_COUNTER: prometheus::CounterVec = prometheus::register_counter_vec!(
        "bragi_http_requests_total",
        "Total number of HTTP requests made.",
        &["handler", "method", "status"]
    )
    .unwrap();

    static ref HTTP_REQ_HISTOGRAM: prometheus::HistogramVec = prometheus::register_histogram_vec!(
        "bragi_http_request_duration_seconds",
        "The HTTP request latencies in seconds.",
        &["handler", "method"],
        prometheus::exponential_buckets(0.001, 1.5, 25).unwrap()
    )
    .unwrap();

    static ref HTTP_IN_FLIGHT: prometheus::Gauge = prometheus::register_gauge!(
        "bragi_http_requests_in_flight",
        "current number of http request being served"
    )
    .unwrap();
}

#[cfg(feature = "metrics")]
pub fn update_metrics(info: warp::log::Info) {
    tracing::trace!(
        "Metric Status: {} - Method: {} - Path: {} - Time: {:?}",
        &info.status().as_u16().to_string(),
        &info.method(),
        &info.path(),
        info.elapsed()
    );
    let method = info.method().to_string();
    let status = info.status().as_u16().to_string();
    let handler = get_ressource_name(info.path());

    HTTP_REQ_HISTOGRAM
        .with_label_values(&[&handler, &method])
        .observe(info.elapsed().as_secs_f64());

    HTTP_COUNTER
        .with_label_values(&[&handler, &method, &status])
        .inc();

    HTTP_IN_FLIGHT.dec();
}

#[cfg(not(feature = "prometheus"))]
pub fn update_metrics(_info: warp::log::Info) {}

#[cfg(feature = "metrics")]
pub fn metrics() -> String {
    let mut buffer = vec![];
    TextEncoder::new()
        .encode(&prometheus::gather(), &mut buffer)
        .unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(not(feature = "metrics"))]
pub fn metrics() -> String {
    "Bragi built without metrics".to_string()
}
