use std::io;
use std::path::PathBuf;

use futures::stream::{Stream, TryStreamExt};

use crate::errors::{ElasticClientError, Result};

use super::ElasticsearchStorage;

#[derive(Debug, Clone, Copy)]
pub enum Template {
    Index,
    Component,
}

pub async fn import(
    client: ElasticsearchStorage,
    path: PathBuf,
    template_type: Template,
) -> Result<()> {
    dir_to_stream(path)
        .await?
        .map_err(ElasticClientError::from)
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
                let template_str = tokio::fs::read_to_string(&template).await?;

                let template_str = template_str.replace("{{ROOT}}", &client.config.index_root);

                let config = config::Config::default()
                    .set_default("elasticsearch.name", template_name)
                    .unwrap()
                    .merge(config::File::from_str(
                        &template_str,
                        config::FileFormat::Json,
                    ))?
                    .clone();

                match template_type {
                    Template::Component => {
                        client
                            .configure(String::from("create component template"), config)
                            .await
                    }
                    Template::Index => {
                        client
                            .configure(String::from("create index template"), config)
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
) -> std::result::Result<
    impl Stream<Item = std::result::Result<PathBuf, io::Error>> + Unpin,
    ElasticClientError,
> {
    let entries = tokio::fs::read_dir(dir.as_path()).await?;

    let stream = tokio_stream::wrappers::ReadDirStream::new(entries);

    Ok(stream.map_ok(|entry| entry.path()))
}
