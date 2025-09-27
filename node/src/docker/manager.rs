//! # Docker Manager
//!
//! Provides an abstraction over the Docker API to manage containerized workloads.
//! Implements the `DockerClient` trait, allowing the runtime to pull images,
//! create, start, stop containers, and retrieve logs.

use crate::{
    docker::errors::DockerError,
    models::{ContainerRuntime, PodRuntime},
};
use async_trait::async_trait;
use bollard::{
    Docker,
    container::LogOutput,
    query_parameters::{
        CreateContainerOptions, CreateImageOptions, InspectContainerOptions, LogsOptions,
        RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
    },
    secret::{ContainerCreateBody, ContainerStateStatusEnum},
};
use bytes::Bytes;
use dashmap::DashSet;
use futures_util::StreamExt;
use futures_util::stream::{BoxStream, TryStreamExt};
use shared::models::pod::Pod;
use std::collections::HashMap;

/// A trait for interacting with container operations needed by the scheduler runtime.
#[async_trait]
pub trait DockerClient: Send + Sync {
    /// Get the current state/status of a container by ID.
    async fn get_container_status(
        &self,
        id: &String,
    ) -> Result<ContainerStateStatusEnum, DockerError>;

    /// Start a pod by pulling its images and launching all specified containers.
    async fn start_pod(&self, pod: Pod) -> Result<PodRuntime, DockerError>;

    /// Stop and remove all containers in a pod
    async fn stop_pod(&self, container_ids: &Vec<String>) -> Result<(), DockerError>;

    /// Fetch the full logs for a container.
    async fn get_logs(&self, container_id: &str) -> Result<String, DockerError>;

    /// Stream logs for a container as a byte stream.
    async fn stream_logs(
        &self,
        id: &str,
    ) -> Result<BoxStream<'static, Result<Bytes, DockerError>>, DockerError>;
}

/// Tracks pulled images and handles bollard docker client
#[derive(Debug)]
pub struct DockerManager {
    images: DashSet<String>,
    client: Docker,
}

impl DockerManager {
    /// Initialize a new `DockerManager` using local Docker defaults.
    pub fn start() -> Result<Self, DockerError> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::ConnectionError(e.to_string()))?;

        Ok(DockerManager {
            images: DashSet::new(),
            client,
        })
    }

    /// Clone-safe getter for the internal Docker client.
    fn client(&self) -> Docker {
        self.client.clone()
    }

    fn has_image(&self, image: &str) -> bool {
        self.images.contains(image)
    }
    fn mark_image_as_pulled(&self, image: String) {
        self.images.insert(image);
    }

    /// Check and pull image if needed
    async fn ensure_image(&self, docker: &Docker, image: &str) -> Result<(), DockerError> {
        if self.has_image(image) {
            return Ok(());
        }

        let options = Some(CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        });

        let mut stream = docker.create_image(options, None, None);

        tracing::debug!(%image, "Pulling");
        while let Some(_status) = stream
            .try_next()
            .await
            .map_err(|e| DockerError::ImagePullError(e.to_string()))?
        {}

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
        let inspection = self
            .client()
            .inspect_container(id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| DockerError::ContainerInspectError(e.to_string()))?;

        Ok(inspection
            .state
            .as_ref()
            .and_then(|s| s.status.clone())
            .unwrap_or_else(|| ContainerStateStatusEnum::EMPTY))
    }

    async fn start_pod(&self, pod: Pod) -> Result<PodRuntime, DockerError> {
        let docker = self.client();
        let mut container_runtimes = HashMap::new();

        // for every container spec in the pod
        for container_spec in &pod.spec.containers {
            self.ensure_image(&docker, &container_spec.image).await?;

            // build unique name
            // NOTE: without namespaces or restarts
            let container_name = format!("cr8s_{}_{}", container_spec.name, pod.metadata.name);

            // build container config from spec
            let config = ContainerCreateBody {
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
                name: Some(container_name.clone()),
                platform: "linux/amd64".to_string(),
            });

            // create and start container
            let container_id = docker
                .create_container(options, config)
                .await
                .map_err(|e| DockerError::ContainerCreationError(e.to_string()))?
                .id;

            docker
                .start_container(&container_id, None::<StartContainerOptions>)
                .await
                .map_err(|e| DockerError::ContainerStartError(e.to_string()))?;

            let status = self.get_container_status(&container_id).await?;

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

        tracing::info!(
            pod=%pod.metadata.name,
            "Started"
        );

        // build final podruntime struct
        Ok(PodRuntime {
            id: pod.metadata.id,
            name: pod.metadata.name,
            containers: container_runtimes,
        })
    }

    async fn stop_pod(&self, container_ids: &Vec<String>) -> Result<(), DockerError> {
        let docker = self.client();

        // stop and remove all containers passing along errors
        for cid in container_ids {
            let id = short_id(cid);
            docker
                .stop_container(cid, None::<StopContainerOptions>)
                .await
                .map_err(|e| {
                    tracing::warn!(id=%id, error=%e, "Failed to stop container");
                    DockerError::ContainerStopError(e.to_string())
                })?;

            docker
                .remove_container(cid, None::<RemoveContainerOptions>)
                .await
                .map_err(|e| {
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
                tail: "all".to_string(),
                ..Default::default()
            }),
        );

        // build log string with basic config
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
                tail: "all".to_string(),
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
