use futures::stream::{Stream, TryStreamExt};
use snafu::{
    futures::{TryFutureExt, TryStreamExt as SnafuTryStreamExt},
    ResultExt, Snafu,
};
use std::path::PathBuf;

use crate::domain::{
    model::error::Error as ModelError, ports::primary::configure_backend::ConfigureBackend,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Config Merge Error: {} [{}]", details, source))]
    ConfigMerge {
        details: String,
        source: config::ConfigError,
    },

    #[snafu(display("IO Error: {} [{}]", source, details))]
    InvalidIO {
        details: String,
        source: std::io::Error,
    },

    #[snafu(display("Backend Error: {}", source))]
    Backend { source: ModelError },
}

#[derive(Debug, Clone, Copy)]
pub enum Template {
    Index,
    Component,
}

pub async fn import<C: Clone + ConfigureBackend>(
    client: C,
    path: PathBuf,
    template_type: Template,
) -> Result<(), Error> {
    dir_to_stream(path)
        .await?
        .try_for_each(|template| {
            tracing::debug!("Importing {:?}", template);
            let template_name = template
                .file_stem()
                .expect("file stem")
                .to_str()
                .expect("template_name")
                .to_string();

            let client = client.clone();
            async move {
                let config = config::Config::builder()
                    .set_default("elasticsearch.name", template_name)
                    .unwrap()
                    .add_source(config::File::new(
                        template.to_str().unwrap(),
                        config::FileFormat::Json,
                    ))
                    .build()
                    .context(ConfigMergeSnafu {
                        details: format!(
                            "could not read template configuration from {}",
                            template.display()
                        ),
                    })?;

                match template_type {
                    Template::Component => {
                        client
                            .configure(String::from("create component template"), config)
                            .context(BackendSnafu)
                            .await
                    }
                    Template::Index => {
                        client
                            .configure(String::from("create index template"), config)
                            .context(BackendSnafu)
                            .await
                    }
                }
            }
        })
        .await
}

// Turns a directory into a Stream of PathBuf
async fn dir_to_stream(
    dir: PathBuf,
) -> Result<impl Stream<Item = Result<PathBuf, Error>> + Unpin, Error> {
    let entries = tokio::fs::read_dir(dir.as_path())
        .await
        .context(InvalidIOSnafu {
            details: format!("{}", dir.display()),
        })?;

    let stream = tokio_stream::wrappers::ReadDirStream::new(entries);

    Ok(stream.map_ok(|entry| entry.path()).context(InvalidIOSnafu {
        details: "could not get path",
    }))
}
