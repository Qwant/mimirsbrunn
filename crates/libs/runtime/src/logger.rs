use snafu::Snafu;
use tracing::metadata::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not init log file: {}", source))]
    InitLog { source: std::io::Error },
}

// FIXME Remove all expects
pub fn logger_init() -> Result<tracing_appender::non_blocking::WorkerGuard, Error> {
    let default_level = LevelFilter::INFO;
    let rust_log =
        std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| default_level.to_string());

    let env_filter = EnvFilter::try_new(rust_log).unwrap_or_else(|err| {
        eprintln!(
            "invalid {}, falling back to level '{}' - {}",
            EnvFilter::DEFAULT_ENV,
            default_level,
            err,
        );
        EnvFilter::new(default_level.to_string())
    });

    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());

    let event_format = tracing_subscriber::fmt::format()
        .with_target(false) // Don't include event targets.
        .compact();

    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .event_format(event_format),
        )
        .with(env_filter);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber.");

    Ok(guard)
}
