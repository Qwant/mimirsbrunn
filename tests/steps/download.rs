use async_trait::async_trait;
use cucumber::given;
use snafu::ResultExt;

use crate::error;
use crate::error::Error;
use crate::state::{GlobalState, State, Step, StepStatus};
use test_harness::download;

// Download OSM

#[given(regex = r"osm file has been downloaded for (\S+)$")]
pub async fn download_osm(state: &mut GlobalState, region: String) {
    state
        .execute_once(DownloadOsm(region))
        .await
        .expect("failed to download OSM file");
}

#[derive(PartialEq)]
pub struct DownloadOsm(pub String);

#[async_trait(?Send)]
impl Step for DownloadOsm {
    async fn execute(&mut self, _state: &State) -> Result<StepStatus, Error> {
        let Self(region) = self;

        download::osm(region)
            .await
            .map(|status| status.into())
            .context(error::DownloadSnafu)
    }
}

// Download bano

#[given(regex = r"bano files have been downloaded for (.+) into (\S+)$")]
pub async fn download_bano(state: &mut GlobalState, departments: String, region: String) {
    let departments = departments
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect();

    state
        .execute_once(DownloadBano {
            departments,
            region,
        })
        .await
        .expect("failed to download OSM file");
}

#[derive(PartialEq)]
pub struct DownloadBano {
    pub departments: Vec<String>,
    pub region: String,
}

#[async_trait(?Send)]
impl Step for DownloadBano {
    async fn execute(&mut self, _state: &State) -> Result<StepStatus, Error> {
        let Self {
            departments,
            region,
        } = self;
        download::bano(region, departments)
            .await
            .map(|status| status.into())
            .context(error::DownloadSnafu)
    }
}

// Download NTFS

#[given(regex = r"ntfs file has been downloaded for (\S+)$")]
pub async fn download_ntfs(state: &mut GlobalState, region: String) {
    state
        .execute_once(DownloadNTFS { region })
        .await
        .expect("failed to download NTFS file");
}

#[derive(Debug, PartialEq)]
pub struct DownloadNTFS {
    pub region: String,
}

#[async_trait(?Send)]
impl Step for DownloadNTFS {
    async fn execute(&mut self, _state: &State) -> Result<StepStatus, Error> {
        let Self { region } = self;
        download::ntfs(region)
            .await
            .map(|status| status.into())
            .context(error::DownloadSnafu)
    }
}
