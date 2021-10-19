use crate::error;
use crate::error::Error;
use crate::state::{State, Step, StepStatus};
use crate::steps::download::DownloadOsm;
use crate::utils::{create_dir_if_not_exists, file_exists};
use async_trait::async_trait;
use common::document::ContainerDocument;
use config::Config;
use cucumber::{t, StepContext, Steps};
use mimir2::adapters::secondary::elasticsearch::ElasticsearchStorage;
use mimir2::domain::model::configuration::root_doctype_dataset;
use mimir2::domain::ports::secondary::storage::Storage;
use places::admin::Admin;
use snafu::ResultExt;
use std::path::PathBuf;

pub fn steps() -> Steps<State> {
    let mut steps: Steps<State> = Steps::new();

    steps.given_regex_async(
        r#"osm file has been processed by cosmogony for ([^\s]*)"#,
        t!(|mut state, ctx| {
            let region = ctx.matches[1].clone();
            assert!(!region.is_empty());

            state
                .execute_once(GenerateCosmogony(region), &ctx)
                .await
                .expect("failed to generate cosmogony file");

            state
        }),
    );

    steps.given_regex_async(
        r#"cosmogony file has been indexed for ([^\s]*)(?: as (.*))?"#,
        t!(|mut state, ctx| {
            let region = ctx.matches[1].clone();
            let dataset = ctx
                .matches
                .get(2)
                .map(|d| {
                    if d.is_empty() {
                        region.to_string()
                    } else {
                        d.to_string()
                    }
                })
                .unwrap_or_else(|| region.clone())
                .clone();
            assert!(!region.is_empty());
            assert!(!dataset.is_empty());
            state
                .execute_once(IndexCosmogony { region, dataset }, &ctx)
                .await
                .expect("failed to index cosmogony file");

            state
        }),
    );

    steps
}

/// Generate a cosmogony file given the region.
///
/// The makes several assumptions:
///  1. The OSM file has previously been downloaded into the expected folder (tests/data/osm)
///  2. The output is a jsonl.gz
///  3. The output will be stored in tests/data/cosmogony
///
/// The OSM file will be processed if:
///  1. The output file is not found
///  2. If the output file is found and the previous step is 'downloaded' (that is it's probably a
///     new OSM file and we need to generate a new cosmogony file.
#[derive(Debug, PartialEq)]
pub struct GenerateCosmogony(pub String);

#[async_trait(?Send)]
impl Step for GenerateCosmogony {
    async fn execute(&mut self, state: &State, _ctx: &StepContext) -> Result<StepStatus, Error> {
        let Self(region) = self;

        let download_state = state
            .status_of(&DownloadOsm(region.to_string()))
            .expect("can't generate cosmogony file without downloading from OSM first");

        let base_path = env!("CARGO_MANIFEST_DIR");

        let input_dir: PathBuf = [base_path, "tests", "fixtures", "osm", region]
            .iter()
            .collect();
        let input_file = input_dir.join(format!("{}-latest.osm.pbf", region));

        let output_dir: PathBuf = [base_path, "tests", "fixtures", "cosmogony", region]
            .iter()
            .collect();
        let output_file = output_dir.join(format!("{}.jsonl.gz", region));
        create_dir_if_not_exists(&output_dir).await?;

        // If the output already exists, and the input is not a new file, then skip the generation
        if file_exists(&output_file).await && download_state == StepStatus::Skipped {
            return Ok(StepStatus::Skipped);
        }

        let cosmogony_path =
            std::env::var("COSMOGONY_EXE").context(error::EnvironmentVariable {
                details: "Could not get cosmogony executable".to_string(),
            })?;

        // TODO: check command status ?
        tokio::process::Command::new(&cosmogony_path)
            .args(["--country-code", "FR"])
            .arg("--input")
            .arg(&input_file)
            .arg("--output")
            .arg(&output_file)
            .spawn()
            .expect("failed to spawn cosmogony")
            .wait()
            .await
            .context(error::InvalidIO {
                details: format!(
                    "failed to generate cosmogony with input {} and output {}",
                    input_file.display(),
                    output_file.display()
                ),
            })?;

        Ok(StepStatus::Done)
    }
}

/// Index a cosmogony file for given region into ES.
///
/// This assumes that a cosmogony file has already been generated before.
#[derive(Debug, PartialEq)]
pub struct IndexCosmogony {
    pub region: String,
    pub dataset: String,
}

#[async_trait(?Send)]
impl Step for IndexCosmogony {
    async fn execute(&mut self, state: &State, ctx: &StepContext) -> Result<StepStatus, Error> {
        let Self { region, dataset } = self;
        let client: &ElasticsearchStorage = ctx.get().expect("could not get ES client");

        let gen_status = state
            .status_of(&GenerateCosmogony(region.to_string()))
            .expect("can't generate cosmogony file without downloading from OSM first");

        // Check if the admin index already exists
        let container = root_doctype_dataset(Admin::static_doc_type(), dataset);

        let index = client
            .find_container(container.clone())
            .await
            .expect("failed at looking up for container");

        // If the previous step has been skipped, then we don't need to index the
        // cosmogony file.
        if gen_status == StepStatus::Skipped && index.is_some() {
            return Ok(StepStatus::Skipped);
        }

        let base_path = env!("CARGO_MANIFEST_DIR");
        let input_dir: PathBuf = [base_path, "tests", "fixtures", "cosmogony", region]
            .iter()
            .collect();
        let input_file = input_dir.join(format!("{}.jsonl.gz", region));

        mimirsbrunn::admin::index_cosmogony(
            input_file.into_os_string().into_string().unwrap(),
            vec!["fr".to_string()],
            Config::builder()
                .add_source(Admin::default_es_container_config())
                .set_override("container.dataset", dataset.to_string())
                .expect("failed to set dataset name")
                .build()
                .expect("failed to build configuration"),
            client,
        )
        .await
        .map_err(|err| Error::Indexing {
            details: format!("could not index cosmogony: {}", err.to_string(),),
        })?;

        Ok(StepStatus::Done)
    }
}
