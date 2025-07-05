use crate::state::{ContainerRuntime, PodRuntime};
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, InspectContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    secret::ContainerStateStatusEnum,
};
use dashmap::DashSet;
use futures_util::stream::TryStreamExt;
use shared::models::PodObject;
use std::{collections::HashMap, fmt};

#[derive(Debug)]
pub enum DockerError {
    ConnectionError(String),
    ImagePullError(String),
    ContainerCreationError(String),
    ContainerStartError(String),
    ContainerInspectError(String),
}

impl fmt::Display for DockerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DockerError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DockerError::ImagePullError(msg) => write!(f, "Image pull error: {}", msg),
            DockerError::ContainerCreationError(msg) => {
                write!(f, "Container creation error: {}", msg)
            }
            DockerError::ContainerStartError(msg) => write!(f, "Container start error: {}", msg),
            DockerError::ContainerInspectError(msg) => {
                write!(f, "Container inspect error: {}", msg)
            }
        }
    }
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

    pub fn client(&self) -> Docker {
        self.client.clone()
    }

    fn has_image(&self, image: &str) -> bool {
        self.images.contains(image)
    }

    fn mark_image_as_pulled(&self, image: String) {
        self.images.insert(image);
    }

    pub async fn get_container_status(
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

    pub async fn start_pod(&self, pod: PodObject) -> Result<PodRuntime, DockerError> {
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
                id=%&container_id[..8.min(container_id.len())],
                status=%status,
                "Started container"
            );

            container_runtimes.insert(
                container_spec.name.clone(),
                ContainerRuntime {
                    id: container_id,
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
