use std::path::PathBuf;

use crate::configuration::{ComponentTemplateConfiguration, IndexTemplateConfiguration};
use config::{Config, File, FileFormat};

use crate::errors::Result;

use super::ElasticSearchClient;

#[derive(Debug, Clone, Copy)]
pub enum Template {
    Index,
    Component,
}

impl ElasticSearchClient {
    pub async fn update_templates(&self) -> Result<()> {
        let path: PathBuf = exporter_config::config_dir()
            .join("elasticsearch")
            .join("templates")
            .join("components");

        tracing::info!("Beginning components imports from {:?}", &path);
        self.import(path, Template::Component).await?;

        let path: PathBuf = exporter_config::config_dir()
            .join("elasticsearch")
            .join("templates")
            .join("indices");

        tracing::info!("Beginning indices imports from {:?}", &path);
        self.import(path, Template::Index).await?;
        Ok(())
    }

    async fn import(&self, path: PathBuf, template_type: Template) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let template_path = entry.path();
            if template_path.is_dir() {
                continue;
            };

            let template_name = format!(
                "{}-{}",
                self.config.index_root,
                template_path
                    .file_stem()
                    .expect("could not fetch template name")
                    .to_str()
                    .expect("invalid template name"),
            );

            let template_str = std::fs::read_to_string(&template_path)?;
            let template_str = template_str.replace("{{ROOT}}", &self.config.index_root);

            let config = Config::builder()
                .add_source(File::from_str(&template_str, FileFormat::Json))
                .set_default("elasticsearch.name", template_name)?
                .build()?;

            match template_type {
                Template::Component => {
                    let config = ComponentTemplateConfiguration::new_from_config(config)?;
                    self.create_component_template(config).await?;
                }
                Template::Index => {
                    let config = IndexTemplateConfiguration::new_from_config(config)?;
                    self.create_index_template(config).await?;
                }
            }
        }

        Ok(())
    }
}
