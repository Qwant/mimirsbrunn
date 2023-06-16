use clap::Parser;
use snafu::{ResultExt, Snafu};

pub mod api;
pub mod handlers;
pub mod prometheus_handler;
pub mod routes;
mod server;
mod settings;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Command Line Interface Error: {}", msg))]
    CLIError { msg: String },
    #[snafu(display("Server Error: {}", source))]
    ServerError {
        #[snafu(backtrace)]
        source: server::Error,
    },
}

fn main() -> Result<(), Error> {
    let opts = settings::Opts::parse();
    match opts.cmd {
        settings::Command::Run => server::run(&opts).context(ServerSnafu),
        settings::Command::Config => server::config(&opts).context(ServerSnafu),
    }
}
