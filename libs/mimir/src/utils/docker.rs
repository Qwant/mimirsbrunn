use std::collections::HashMap;
use std::path::PathBuf;

use bollard::{
    container::{
        Config as BollardConfig, CreateContainerOptions, ListContainersOptions,
        StartContainerOptions,
    },
    errors::Error as BollardError,
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
    Docker,
};
use elasticsearch::{
    cluster::ClusterDeleteComponentTemplateParts,
    http::transport::BuildError as TransportBuilderError,
    indices::{IndicesDeleteAliasParts, IndicesDeleteIndexTemplateParts, IndicesDeleteParts},
    Error as ElasticsearchError,
};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use tokio::time::{sleep, Duration};

use crate::{
    adapters::{
        primary::templates,
        secondary::elasticsearch::{remote, ElasticsearchStorageConfig},
    },
    domain::ports::secondary::remote::{Error as RemoteError, Remote},
};

pub async fn initialize() -> Result<(), Error> {
    initialize_with_param(true).await
}

/// Initializes a docker container for testing
/// It will see if a docker container is available with the default name
/// If there is no container, it will create one.
/// If there is already a container, and the parameter cleanup is true,
/// then all the indices found on that Elasticsearch are wiped out.
/// Once the container is available, a connection is attempted, to make
/// sure subsequent calls to that Elasticsearch will be successful.
pub async fn initialize_with_param(cleanup: bool) -> Result<(), Error> {
    let mut docker = DockerWrapper::new();

    if docker.docker_config.enable && !docker.is_container_available().await? {
        docker.create_container().await?;

        if !docker.is_container_available().await? {
            return Err(Error::Misc {
                msg: format!(
                    "Cannot get docker {} available",
                    docker.docker_config.container.name
                ),
            });
        }
    } else if cleanup {
        docker.cleanup().await?;
    }

    let config = ElasticsearchStorageConfig::default_testing();

    let client = remote::connection_pool_url(&config.url)
        .conn(config)
        .await
        .context(ElasticsearchConnectionSnafu)?;

    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "config",
        "elasticsearch",
        "templates",
        "components",
    ]
    .iter()
    .collect();

    templates::import(client.clone(), path, templates::Template::Component)
        .await
        .context(TemplateLoadingSnafu)?;

    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "config",
        "elasticsearch",
        "templates",
        "indices",
    ]
    .iter()
    .collect();

    templates::import(client, path, templates::Template::Index)
        .await
        .context(TemplateLoadingSnafu)
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Connection to docker socket: {}", source))]
    DockerConnection { source: BollardError },

    #[snafu(display("Connection to elasticsearch: {}", source))]
    ElasticsearchConnection { source: RemoteError },

    #[snafu(display("docker version: {}", source))]
    Version { source: BollardError },

    #[snafu(display("url parsing error: {}", source))]
    UrlParse { source: url::ParseError },

    #[snafu(display("elasticsearch transport error: {}", source))]
    ElasticsearchTransport { source: TransportBuilderError },

    #[snafu(display("elasticsearch client error: {}", source))]
    ElasticsearchClient { source: ElasticsearchError },

    #[snafu(display("docker error: {}", source))]
    DockerEngine { source: BollardError },

    #[snafu(display("error: {}", msg))]
    Misc { msg: String },

    #[snafu(display("Template Loading Error: {}", source))]
    TemplateLoading { source: templates::Error },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerVersion {
    pub major: usize,
    pub minor: usize,
}

impl From<DockerVersion> for bollard::ClientVersion {
    fn from(version: DockerVersion) -> bollard::ClientVersion {
        bollard::ClientVersion {
            major_version: version.major,
            minor_version: version.minor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub image: String,
    pub name: String,
    pub memory: i64,
    pub vars: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    pub container: ContainerConfig,
    pub enable: bool,
    pub timeout: u64,
    pub version: DockerVersion,
    pub container_wait: u64,
    pub elasticsearch_wait: u64,
    pub cleanup_wait: u64,
}

impl Default for DockerConfig {
    /// We retrieve the docker configuration from ./config/elasticsearch/default.
    fn default() -> Self {
        let config = common::config::config_from(
            &PathBuf::from("config"),
            &["elasticsearch"],
            None,
            None,
            vec![],
        );

        config
            .expect("cannot build the configuration for testing from config")
            .get("elasticsearch")
            .expect("expected elasticsearch section in configuration from config")
    }
}

pub struct DockerWrapper {
    ports: Vec<(u32, u32)>, // list of ports to publish (host port, container port)
    docker_config: DockerConfig,
}

impl DockerConfig {
    pub fn default_testing() -> Self {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config");

        let config = common::config::config_from(
            config_dir.as_path(),
            &["docker"],
            "testing",
            "MIMIR_TEST",
            vec![],
        );

        config
            .unwrap_or_else(|_| {
                panic!(
                    "cannot build the configuration for testing from {}",
                    config_dir.display(),
                )
            })
            .get("docker")
            .unwrap_or_else(|_| {
                panic!(
                    "expected docker section in configuration from {}",
                    config_dir.display(),
                )
            })
    }
    pub fn connect(&self) -> Result<Docker, Error> {
        Docker::connect_with_unix(
            "unix:///var/run/docker.sock",
            self.timeout,
            &self.version.clone().into(),
        )
        .context(DockerConnectionSnafu)
    }
}

impl Default for DockerWrapper {
    fn default() -> Self {
        let elasticsearch_config = ElasticsearchStorageConfig::default_testing();
        let docker_config = DockerConfig::default_testing();

        let port = elasticsearch_config
            .url
            .port()
            .expect("expected port in elasticsearch url");

        let offset: u32 = (port - 9000).into();
        DockerWrapper {
            ports: vec![(9000 + offset, 9200), (9300 + offset, 9300)],
            docker_config,
        }
    }
}

impl DockerWrapper {
    pub fn new() -> DockerWrapper {
        DockerWrapper::default()
    }

    // Returns true if the container self.docker_config.container.name is running
    pub async fn is_container_available(&mut self) -> Result<bool, Error> {
        let docker = self.docker_config.connect()?;

        let docker = &docker.negotiate_version().await.context(VersionSnafu)?;

        docker.version().await.context(VersionSnafu)?;

        let mut filters = HashMap::new();
        filters.insert("name", vec![self.docker_config.container.name.as_str()]);

        let options = Some(ListContainersOptions {
            all: false, // only running containers
            filters,
            ..Default::default()
        });

        let containers = docker
            .list_containers(options)
            .await
            .context(DockerEngineSnafu)?;

        Ok(!containers.is_empty())
    }

    // If the container is already created, then start it.
    // If it is not created, then create it and start it.
    pub async fn create_container(&mut self) -> Result<(), Error> {
        let docker = self.docker_config.connect()?;

        let docker = docker.negotiate_version().await.context(VersionSnafu)?;

        let _ = docker.version().await.context(VersionSnafu);

        let mut filters = HashMap::new();
        filters.insert("name", vec![self.docker_config.container.name.as_str()]);

        let options = Some(ListContainersOptions {
            all: true, // only running containers
            filters,
            ..Default::default()
        });

        let containers = docker
            .list_containers(options)
            .await
            .context(DockerEngineSnafu)?;

        if containers.is_empty() {
            let options = CreateContainerOptions {
                name: &self.docker_config.container.name,
            };

            let mut port_bindings = HashMap::new();
            for (host_port, container_port) in self.ports.iter() {
                port_bindings.insert(
                    format!("{}/tcp", &container_port),
                    Some(vec![PortBinding {
                        host_ip: Some(String::from("0.0.0.0")),
                        host_port: Some(host_port.to_string()),
                    }]),
                );
            }

            let host_config = HostConfig {
                port_bindings: Some(port_bindings),
                memory: Some(self.docker_config.container.memory * 1024 * 1024),
                ..Default::default()
            };

            let mut exposed_ports = HashMap::new();
            self.ports.iter().for_each(|(_, container)| {
                let v: HashMap<(), ()> = HashMap::new();
                exposed_ports.insert(format!("{}/tcp", container), v);
            });

            let env = Some(self.docker_config.container.vars.clone()).and_then(|vars| {
                if vars.is_empty() {
                    None
                } else {
                    Some(vars)
                }
            });

            let config = BollardConfig {
                image: Some(self.docker_config.container.image.clone()),
                exposed_ports: Some(exposed_ports),
                host_config: Some(host_config),
                env,
                ..Default::default()
            };

            docker
                .create_image(
                    Some(CreateImageOptions {
                        from_image: self.docker_config.container.image.clone(),
                        ..Default::default()
                    }),
                    None,
                    None,
                )
                .try_collect::<Vec<_>>()
                .await
                .context(DockerEngineSnafu)?;

            let _ = docker
                .create_container(Some(options), config)
                .await
                .context(DockerEngineSnafu)?;

            sleep(Duration::from_millis(self.docker_config.container_wait)).await;
        }
        docker
            .start_container(
                &self.docker_config.container.name,
                None::<StartContainerOptions<String>>,
            )
            .await
            .context(DockerEngineSnafu)?;

        sleep(Duration::from_millis(self.docker_config.elasticsearch_wait)).await;

        Ok(())
    }

    /// This function cleans up the Elasticsearch
    async fn cleanup(&mut self) -> Result<(), Error> {
        let pool = remote::connection_test_pool();

        let storage = pool
            .conn(ElasticsearchStorageConfig::default_testing())
            .await
            .context(ElasticsearchConnectionSnafu)?;

        let _ = storage
            .client
            .indices()
            .delete(IndicesDeleteParts::Index(&["*"]))
            .request_timeout(storage.config.timeout)
            .send()
            .await
            .context(ElasticsearchClientSnafu)?;

        let _ = storage
            .client
            .indices()
            .delete_alias(IndicesDeleteAliasParts::IndexName(&["*"], &["*"]))
            .request_timeout(storage.config.timeout)
            .send()
            .await
            .context(ElasticsearchClientSnafu)?;

        let _ = storage
            .client
            .indices()
            .delete_index_template(IndicesDeleteIndexTemplateParts::Name("munin_*"))
            .request_timeout(storage.config.timeout)
            .send()
            .await
            .context(ElasticsearchClientSnafu)?;

        let _ = storage
            .client
            .cluster()
            .delete_component_template(ClusterDeleteComponentTemplateParts::Name("mimir-*"))
            .request_timeout(storage.config.timeout)
            .send()
            .await
            .context(ElasticsearchClientSnafu)?;

        sleep(Duration::from_millis(self.docker_config.cleanup_wait)).await;
        Ok(())
    }

    async fn _drop(&mut self) {
        if std::env::var("DONT_KILL_THE_WHALE") == Ok("1".to_string()) {
            println!(
                "the docker won't be stoped at the end, you can debug it.
                Note: ES has been mapped to the port 9242 in you localhost
                manually stop and rm the container mimirsbrunn_tests after debug"
            );
            return;
        }
        let docker = self
            .docker_config
            .connect()
            .expect("docker engine connection");

        let options = Some(bollard::container::StopContainerOptions { t: 0 });
        docker
            .stop_container(&self.docker_config.container.name, options)
            .await
            .expect("stop container");

        let options = Some(bollard::container::RemoveContainerOptions {
            force: true,
            ..Default::default()
        });

        docker
            .remove_container(&self.docker_config.container.name, options)
            .await
            .expect("remove container");
    }
}
