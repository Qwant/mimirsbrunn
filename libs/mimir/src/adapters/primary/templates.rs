use crate::{
    adapters::secondary::elasticsearch::ElasticsearchStorage,
    domain::{
        model::error::Error as ModelError, ports::primary::configure_backend::ConfigureBackend,
    },
};
use futures::stream::{Stream, TryStreamExt};
use snafu::{
    futures::{TryFutureExt, TryStreamExt as SnafuTryStreamExt},
    ResultExt, Snafu,
};
use std::path::PathBuf;

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

pub async fn import(
    client: ElasticsearchStorage,
    path: PathBuf,
    template_type: Template,
) -> Result<(), Error> {
    dir_to_stream(path)
        .await?
        .try_for_each(|template| {
            tracing::debug!("Importing {:?}", template);
            let client = client.clone();

            let template_name = format!(
                "{}-{}",
                client.config.index_root,
                template
                    .file_stem()
                    .expect("could not fetch template name")
                    .to_str()
                    .expect("invalid template name"),
            );

            async move {
                let template_str =
                    tokio::fs::read_to_string(&template)
                        .await
                        .context(InvalidIOSnafu {
                            details: "could not open template",
                        })?;

                let template_str = template_str.replace("{{ROOT}}", &client.config.index_root);

                let config = config::Config::default()
                    .set_default("elasticsearch.name", template_name)
                    .unwrap()
                    .merge(config::File::from_str(
                        &template_str,
                        config::FileFormat::Json,
                    ))
                    .context(ConfigMergeSnafu {
                        details: format!(
                            "could not read template configuration from {}",
                            template.display()
                        ),
                    })?
                    .clone();

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
