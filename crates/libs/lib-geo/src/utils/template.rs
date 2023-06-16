use elastic_client::templates;
use elastic_client::ElasticsearchStorage;
use std::path::PathBuf;

pub async fn update_templates(
    client: &ElasticsearchStorage,
    db_file: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = db_file
        .join("elasticsearch")
        .join("templates")
        .join("components");

    tracing::info!("Beginning components imports from {:?}", &path);
    templates::import(client.clone(), path, templates::Template::Component)
        .await
        .map_err(Box::new)?;

    let path: PathBuf = db_file
        .join("elasticsearch")
        .join("templates")
        .join("indices");

    tracing::info!("Beginning indices imports from {:?}", &path);
    templates::import(client.clone(), path, templates::Template::Index)
        .await
        .map_err(Box::new)?;
    Ok(())
}
