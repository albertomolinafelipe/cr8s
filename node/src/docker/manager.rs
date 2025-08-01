use crate::{
    docker::DockerError,
    state::{ContainerRuntime, PodRuntime},
};
use async_trait::async_trait;
use bollard::{
    Docker,
    container::{
        Config, CreateContainerOptions, InspectContainerOptions, LogOutput, LogsOptions,
        StartContainerOptions,
    },
    image::CreateImageOptions,
    secret::ContainerStateStatusEnum,
};
use bytes::Bytes;
use dashmap::DashSet;
use futures_util::StreamExt;
use futures_util::stream::{BoxStream, TryStreamExt};
use shared::models::PodObject;
use std::collections::HashMap;

#[async_trait]
pub trait DockerClient: Send + Sync {
    async fn get_container_status(
        &self,
        id: &String,
    ) -> Result<ContainerStateStatusEnum, DockerError>;
    async fn start_pod(&self, pod: PodObject) -> Result<PodRuntime, DockerError>;
    async fn stop_pod(&self, container_ids: &Vec<String>) -> Result<(), DockerError>;
    async fn get_logs(&self, container_id: &str) -> Result<String, DockerError>;
    async fn stream_logs(
        &self,
        id: &str,
    ) -> Result<BoxStream<'static, Result<bytes::Bytes, DockerError>>, DockerError>;
}

#[derive(Debug)]
pub struct DockerManager {
    images: DashSet<String>,
    client: Docker,
}

impl DockerManager {
    pub fn start() -> Result<Self, DockerError> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::ConnectionError(e.to_string()))?;

        Ok(DockerManager {
            images: DashSet::new(),
            client,
        })
    }

    fn client(&self) -> Docker {
        self.client.clone()
    }

    fn has_image(&self, image: &str) -> bool {
        self.images.contains(image)
    }

    fn mark_image_as_pulled(&self, image: String) {
        self.images.insert(image);
    }

    async fn ensure_image(&self, docker: &Docker, image: &str) -> Result<(), DockerError> {
        if self.has_image(image) {
            return Ok(());
        }

        let options = Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        });

        let mut stream = docker.create_image(options, None, None);

        while let Some(_status) = stream
            .try_next()
            .await
            .map_err(|e| DockerError::ImagePullError(e.to_string()))?
        {}

        tracing::info!(image=%image, "Pulled container");
        self.mark_image_as_pulled(image.to_string());

        Ok(())
    }
}

#[async_trait]
impl DockerClient for DockerManager {
    async fn get_container_status(
        &self,
        id: &String,
    ) -> Result<ContainerStateStatusEnum, DockerError> {
        let docker = self.client();
        let inspection = docker
            .inspect_container(id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| DockerError::ContainerInspectError(e.to_string()))?;
        Ok(inspection
            .state
            .as_ref()
            .and_then(|s| s.status.clone())
            .unwrap_or_else(|| ContainerStateStatusEnum::EMPTY))
    }
    async fn start_pod(&self, pod: PodObject) -> Result<PodRuntime, DockerError> {
        let docker = self.client();
        let mut container_runtimes = HashMap::new();

        for container_spec in &pod.spec.containers {
            self.ensure_image(&docker, &container_spec.image).await?;

            let container_name = format!("pod_{}_{}", pod.metadata.user.name, container_spec.name);

            let config = Config {
                image: Some(container_spec.image.clone()),
                env: container_spec.env.as_ref().map(|envs| {
                    envs.iter()
                        .map(|env| format!("{}={}", env.name, env.value))
                        .collect()
                }),
                exposed_ports: container_spec.ports.as_ref().map(|ports| {
                    ports
                        .iter()
                        .map(|p| (format!("{}/tcp", p.container_port), HashMap::new()))
                        .collect()
                }),
                ..Default::default()
            };

            let options = Some(CreateContainerOptions {
                name: &container_name,
                platform: None,
            });

            let create_response = docker
                .create_container(options, config)
                .await
                .map_err(|e| DockerError::ContainerCreationError(e.to_string()))?;

            let container_id = create_response.id;

            docker
                .start_container(&container_id, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| DockerError::ContainerStartError(e.to_string()))?;

            let inspection = docker
                .inspect_container(&container_id, None::<InspectContainerOptions>)
                .await
                .map_err(|e| DockerError::ContainerInspectError(e.to_string()))?;

            let status = inspection
                .state
                .as_ref()
                .and_then(|s| s.status.clone())
                .unwrap_or_else(|| ContainerStateStatusEnum::EMPTY);

            tracing::debug!(
                id=%short_id(&container_id),
                status=%status,
                "Started container"
            );

            container_runtimes.insert(
                container_spec.name.clone(),
                ContainerRuntime {
                    id: container_id,
                    spec_name: container_spec.name.clone(),
                    name: container_name,
                    status,
                },
            );
        }

        Ok(PodRuntime {
            id: pod.id,
            name: pod.metadata.user.name,
            containers: container_runtimes,
        })
    }
    async fn stop_pod(&self, container_ids: &Vec<String>) -> Result<(), DockerError> {
        let docker = self.client();

        for cid in container_ids {
            let id = short_id(cid);
            docker.stop_container(cid, None).await.map_err(|e| {
                tracing::warn!(id=%id, error=%e, "Failed to stop container");
                DockerError::ContainerStopError(e.to_string())
            })?;
            tracing::debug!(id=%id, "Stopped container");
            docker.remove_container(cid, None).await.map_err(|e| {
                tracing::warn!(id=%id, error=%e, "Failed to remove container");
                DockerError::ContainerRemovalError(e.to_string())
            })?;
            tracing::debug!(id=%id, "Removed container");
        }

        Ok(())
    }
    async fn get_logs(&self, container_id: &str) -> Result<String, DockerError> {
        let docker = self.client();
        let mut logs_stream = docker.logs(
            container_id,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                follow: false,
                tail: "all",
                ..Default::default()
            }),
        );

        let mut output = String::new();
        while let Some(chunk) = logs_stream
            .try_next()
            .await
            .map_err(|e| DockerError::LogsError(e.to_string()))?
        {
            match chunk {
                LogOutput::StdOut { message }
                | LogOutput::StdErr { message }
                | LogOutput::Console { message } => {
                    output.push_str(&String::from_utf8_lossy(&message));
                }
                _ => {}
            }
        }

        Ok(output)
    }
    async fn stream_logs(
        &self,
        id: &str,
    ) -> Result<BoxStream<'static, Result<Bytes, DockerError>>, DockerError> {
        let docker = self.client();

        let mut logs_stream = docker.logs(
            id,
            Some(LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                timestamps: false,
                tail: "all".to_string(),
                since: 0,
                ..Default::default()
            }),
        );

        let stream = async_stream::stream! {
            while let Some(item) = logs_stream.next().await {
                match item {
                    Ok(LogOutput::StdOut { message })
                        | Ok(LogOutput::StdErr { message })
                        | Ok(LogOutput::Console { message }) => {
                            yield Ok(message);
                        }
                    Ok(_) => continue,
                    Err(e) => {
                        yield Err(DockerError::StreamLogsError(e.to_string()));
                        break;
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

fn short_id(id: &str) -> &str {
    id.get(0..8).unwrap_or(id)
}
