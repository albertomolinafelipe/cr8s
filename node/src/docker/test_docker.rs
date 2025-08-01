use crate::docker::DockerError;
use crate::docker::manager::DockerClient;
use crate::state::{ContainerRuntime, PodRuntime};
use async_trait::async_trait;
use bollard::secret::ContainerStateStatusEnum;
use shared::models::PodObject;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TestDocker {
    containers: Arc<Mutex<HashMap<String, ContainerStateStatusEnum>>>,
    pub fail_start: bool,
    pub fail_stop: bool,
    pub fail_remove: bool,
    pub failt_get_status: bool,
}

impl TestDocker {
    pub fn new() -> Self {
        Self {
            containers: Arc::new(Mutex::new(HashMap::new())),
            fail_start: false,
            fail_stop: false,
            fail_remove: false,
            failt_get_status: false,
        }
    }

    pub async fn add_fake_container(&self, id: &str, status: ContainerStateStatusEnum) {
        let mut lock = self.containers.lock().await;
        lock.insert(id.to_string(), status);
    }

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
}
