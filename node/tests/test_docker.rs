//! Test implementation of the DockerClient trait for use in unit tests.
//! Simulates container lifecycle behavior with configurable error injection.

use async_trait::async_trait;
use bollard::secret::ContainerStateStatusEnum;
use futures_util::stream::BoxStream;
use r8sagt::docker::errors::DockerError;
use r8sagt::docker::manager::DockerClient;
use r8sagt::models::{ContainerRuntime, PodRuntime};
use shared::models::PodObject;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// A mock Docker client for testing, simulating container operations with optional failure modes.
#[derive(Debug, Clone)]
pub struct TestDocker {
    containers: Arc<Mutex<HashMap<String, ContainerStateStatusEnum>>>,
    pub fail_start: bool,
    pub fail_stop: bool,
    pub fail_remove: bool,
    pub failt_get_status: bool,
}

impl TestDocker {
    /// Create a new instance of the test Docker client with no containers or failures set.
    pub fn new() -> Self {
        Self {
            containers: Arc::new(Mutex::new(HashMap::new())),
            fail_start: false,
            fail_stop: false,
            fail_remove: false,
            failt_get_status: false,
        }
    }

    /// Insert a fake container with the given ID and status into the mock state.
    pub async fn add_fake_container(&self, id: &str, status: ContainerStateStatusEnum) {
        let mut lock = self.containers.lock().await;
        lock.insert(id.to_string(), status);
    }

    /// Generate a mock container ID by appending a UUID to the container name.
    fn generate_container_id(name: &str) -> String {
        format!("{}-{}", name, Uuid::new_v4())
    }
}

#[async_trait]
impl DockerClient for TestDocker {
    async fn get_container_status(
        &self,
        id: &String,
    ) -> Result<ContainerStateStatusEnum, DockerError> {
        if self.failt_get_status {
            return Err(DockerError::ContainerInspectError("Forced error".into()));
        }

        let containers = self.containers.lock().await;
        Ok(containers
            .get(id)
            .cloned()
            .unwrap_or(ContainerStateStatusEnum::EMPTY))
    }

    async fn start_pod(&self, pod: PodObject) -> Result<PodRuntime, DockerError> {
        if self.fail_start {
            return Err(DockerError::ContainerStartError("Forced error".into()));
        }

        let mut containers_runtime = HashMap::new();
        let mut container_states = self.containers.lock().await;

        for container_spec in &pod.spec.containers {
            let container_id = Self::generate_container_id(&container_spec.name);

            container_states.insert(container_id.clone(), ContainerStateStatusEnum::RUNNING);

            containers_runtime.insert(
                container_spec.name.clone(),
                ContainerRuntime {
                    id: container_id.clone(),
                    spec_name: container_spec.name.clone(),
                    name: container_spec.name.clone(),
                    status: ContainerStateStatusEnum::RUNNING,
                },
            );
        }

        Ok(PodRuntime {
            id: pod.id,
            name: pod.metadata.user.name,
            containers: containers_runtime,
        })
    }

    async fn stop_pod(&self, container_ids: &Vec<String>) -> Result<(), DockerError> {
        if self.fail_stop {
            return Err(DockerError::ContainerStopError("Forced error".into()));
        }

        if self.fail_remove {
            return Err(DockerError::ContainerRemovalError("Forced error".into()));
        }

        let mut containers = self.containers.lock().await;
        for id in container_ids {
            containers.remove(id);
        }

        Ok(())
    }

    async fn get_logs(&self, _container_id: &str) -> Result<String, DockerError> {
        Ok("Here, your logs".to_string())
    }

    async fn stream_logs(
        &self,
        _id: &str,
    ) -> Result<BoxStream<'static, Result<bytes::Bytes, DockerError>>, DockerError> {
        Err(DockerError::StreamLogsError("Forced".into()))
    }
}
