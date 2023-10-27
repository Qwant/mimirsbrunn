use crate::wait::ReadyCondition;
use bollard::Docker;

pub struct Container {
    pub image: String,
    pub name: String,
    pub client: Docker,
    pub env: Vec<(String, String)>,
    pub ready_condition: ReadyCondition,
    pub memory: Option<i64>,
    pub memory_swap: Option<i64>,
    pub exposed_port: Vec<(u16, u16)>,
}

impl Container {
    pub async fn run(&self) -> anyhow::Result<()> {
        self.create_image().await?;
        let config = self.build_config().await?;
        self.create_container(config).await?;
        self.start_container().await?;
        self.wait_for_ready_condition().await?;
        self.log();

        Ok(())
    }

    // Returns true if the container self.docker_config.container.name is running
    pub async fn is_running(&self) -> bool {
        let Ok(inspect_response) = self.client.inspect_container(&self.name, None).await else {
            return false;
        };

        inspect_response
            .state
            .and_then(|state| state.running)
            .unwrap_or(false)
    }
}

pub mod inner {
    use anyhow::anyhow;
    use bollard::container::{Config, CreateContainerOptions, LogsOptions, StartContainerOptions};
    use bollard::image::CreateImageOptions;
    use bollard::models::{HealthStatusEnum, HostConfig, PortBinding, PortMap};
    use std::time::Duration;

    use crate::container::Container;
    use crate::port::Port;
    use crate::wait::ReadyCondition;
    use futures_util::{StreamExt, TryStreamExt};

    impl Container {
        pub(crate) async fn create_container(&self, config: Config<String>) -> anyhow::Result<()> {
            self.client
                .create_container(
                    Some(CreateContainerOptions {
                        name: &self.name,
                        platform: None,
                    }),
                    config,
                )
                .await?;
            Ok(())
        }

        pub(crate) async fn start_container(&self) -> anyhow::Result<()> {
            self.client
                .start_container(&self.name, None::<StartContainerOptions<String>>)
                .await?;
            Ok(())
        }

        pub(crate) async fn wait_for_ready_condition(&self) -> anyhow::Result<()> {
            match &self.ready_condition {
                ReadyCondition::_Stdout(expected) => {
                    let mut stream = self.client.logs::<String>(
                        &self.name,
                        Some(LogsOptions {
                            follow: true,
                            stdout: true,
                            stderr: true,
                            ..Default::default()
                        }),
                    );

                    while let Some(msg) = stream.next().await {
                        let msg = msg.unwrap().to_string();
                        if msg.contains(expected) {
                            return Ok(());
                        }
                    }

                    return Err(anyhow!("end of logs"));
                }
                ReadyCondition::HttpPull {
                    url,
                    expect,
                    interval,
                } => loop {
                    let Ok(response) = reqwest::get(url).await else {
                        tokio::time::sleep(*interval).await;
                        continue;
                    };

                    let Ok(text) = response.text().await else {
                        tokio::time::sleep(*interval).await;
                        continue;
                    };

                    if text.contains(expect) {
                        break;
                    }

                    tokio::time::sleep(*interval).await;
                },
                ReadyCondition::_Healthy => loop {
                    let inspect = self.client.inspect_container(&self.name, None).await?;
                    let status = inspect
                        .state
                        .and_then(|state| state.health)
                        .and_then(|health| health.status);
                    match status {
                        None | Some(HealthStatusEnum::EMPTY) | Some(HealthStatusEnum::NONE) => {
                            panic!("Container does not have a healthcheck")
                        }
                        Some(HealthStatusEnum::STARTING) => {
                            tokio::time::sleep(Duration::from_millis(50)).await
                        }
                        Some(HealthStatusEnum::HEALTHY) => break,
                        Some(HealthStatusEnum::UNHEALTHY) => {
                            panic!("Container health check failed")
                        }
                    }
                },
            }

            Ok(())
        }

        pub(crate) fn log(&self) {
            let name = self.name.clone();

            let mut stream = self.client.logs::<String>(
                &name,
                Some(LogsOptions {
                    follow: true,
                    stdout: true,
                    stderr: true,
                    ..Default::default()
                }),
            );

            tokio::spawn(async move {
                println!();
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(msg) => print!("{msg}"),
                        Err(err) => print!("{err}"),
                    };
                }
            });
        }

        pub(crate) async fn create_image(&self) -> anyhow::Result<()> {
            let _ = self
                .client
                .create_image(
                    Some(CreateImageOptions {
                        from_image: self.image.as_str(),
                        ..Default::default()
                    }),
                    None,
                    None,
                )
                .try_collect::<Vec<_>>()
                .await?;

            Ok(())
        }

        pub(crate) async fn build_config(&self) -> anyhow::Result<Config<String>> {
            let mut portmap = PortMap::new();

            for (internal, external) in &self.exposed_port {
                let internal = Port::Tcp(*internal);
                let external = Port::Tcp(*external);

                portmap.insert(
                    external.to_string(),
                    Some(vec![PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some(internal.to_string()),
                    }]),
                );
            }

            let host_config = HostConfig {
                port_bindings: Some(portmap),
                memory: self.memory,
                memory_swap: self.memory_swap,
                ..Default::default()
            };

            let env: Vec<String> = self.env.iter().map(|(k, v)| format!("{k}={v}")).collect();

            let config = Config {
                image: Some(self.image.clone()),
                env: Some(env),
                host_config: Some(host_config),
                ..Default::default()
            };

            Ok(config)
        }
    }
}
