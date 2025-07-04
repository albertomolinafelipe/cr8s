use crate::state::{ContainerRuntime, PodRuntime};
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, InspectContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
};
use dashmap::DashSet;
use futures_util::stream::TryStreamExt;
use shared::models::PodObject;
use std::collections::HashMap;

#[derive(Debug)]
pub struct DockerManager {
    images: DashSet<String>,
    client: Docker,
}

impl DockerManager {
    pub fn new() -> Self {
        DockerManager {
            images: DashSet::new(),
            client: Docker::connect_with_local_defaults()
                .expect("Failed to connect to Docker daemon"),
        }
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

    pub async fn start_pod(&self, pod: PodObject) -> PodRuntime {
        let docker = self.client();
        let mut container_runtimes = Vec::new();
        for container_spec in &pod.spec.containers {
            self.ensure_image(&docker, &container_spec.image).await;

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
                .expect("Failed to create container");

            let container_id = create_response.id;

            docker
                .start_container(&container_id, None::<StartContainerOptions<String>>)
                .await
                .expect("Failed to start container");

            let inspection = docker
                .inspect_container(&container_id, None::<InspectContainerOptions>)
                .await
                .expect("Failed to inspect container");

            let status = inspection
                .state
                .as_ref()
                .and_then(|s| s.status.clone())
                .unwrap();

            tracing::info!(
                id=%&container_id[..8.min(container_id.len())],
                status=%status,
                "Started container"
            );
            container_runtimes.push(ContainerRuntime {
                id: container_id,
                name: container_name,
                status,
            });
        }

        PodRuntime {
            id: pod.id,
            containers: container_runtimes,
        }
    }

    async fn ensure_image(&self, docker: &Docker, image: &str) {
        if self.has_image(image) {
            return;
        }

        let options = Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        });

        let mut stream = docker.create_image(options, None, None);
        while let Some(_status) = stream.try_next().await.unwrap_or(None) {
            // logging?
        }
        tracing::info!(image=%image, "Pulled container");
        self.mark_image_as_pulled(image.to_string());
    }
}
