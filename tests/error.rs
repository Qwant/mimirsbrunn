use snafu::Snafu;

use test_harness::{bano as bano_test, cosmogony, download, osm};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Download Error: {}", source))]
    Download { source: download::Error },

    #[snafu(display("Generate Cosmogony Error: {}", source))]
    GenerateCosmogony { source: cosmogony::Error },

    #[snafu(display("Index Cosmogony Error: {}", source))]
    IndexCosmogony { source: cosmogony::Error },

    #[snafu(display("Index Bano Error: {}", source))]
    IndexBano { source: bano_test::Error },

    #[snafu(display("Index Osm Error: {}", source))]
    IndexOsm { source: osm::Error },

    #[snafu(display("Environment Variable Error: {} ({})", details, source))]
    EnvironmentVariable {
        details: String,
        source: std::env::VarError,
    },

    #[snafu(display("Miscellaneous Error: {}", details))]
    Miscellaneous { details: String },
}
