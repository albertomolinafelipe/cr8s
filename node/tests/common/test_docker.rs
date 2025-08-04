//! Test implementation of the DockerClient trait for use in unit tests.
//! Simulates container lifecycle behavior with configurable error injection.

use async_trait::async_trait;
use bollard::secret::ContainerStateStatusEnum;
use dashmap::DashMap;
use futures_util::lock::Mutex;
use futures_util::stream::BoxStream;
use r8sagt::docker::errors::DockerError;
use r8sagt::docker::manager::DockerClient;
use r8sagt::models::{ContainerRuntime, PodRuntime};
use shared::models::PodObject;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A mock Docker client for testing, simulating container operations with optional failure modes.

#[derive(Debug, Clone)]
pub struct TestDocker {
    containers: Arc<DashMap<String, ContainerStateStatusEnum>>,
    pub fail_start: bool,
    pub fail_stop: bool,
    pub fail_remove: bool,
    pub failt_get_status: bool,

    pub get_container_status_calls: Arc<Mutex<Vec<String>>>,
    pub start_pod_calls: Arc<Mutex<Vec<PodObject>>>,
    pub stop_pod_calls: Arc<Mutex<Vec<Vec<String>>>>,
    pub get_logs_calls: Arc<Mutex<Vec<String>>>,
    pub stream_logs_calls: Arc<Mutex<Vec<String>>>,
}

impl TestDocker {
    pub fn new() -> Self {
        Self {
            containers: Arc::new(DashMap::new()),
            fail_start: false,
            fail_stop: false,
            fail_remove: false,
            failt_get_status: false,
            get_container_status_calls: Arc::new(Mutex::new(Vec::new())),
            start_pod_calls: Arc::new(Mutex::new(Vec::new())),
            stop_pod_calls: Arc::new(Mutex::new(Vec::new())),
            get_logs_calls: Arc::new(Mutex::new(Vec::new())),
            stream_logs_calls: Arc::new(Mutex::new(Vec::new())),
        }
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
        // Record argument (clone String)
        self.get_container_status_calls
            .lock()
            .await
            .push(id.clone());

        if self.failt_get_status {
            return Err(DockerError::ContainerInspectError("Forced error".into()));
        }

        Ok(self
            .containers
            .get(id)
            .map(|entry| entry.clone())
            .unwrap_or(ContainerStateStatusEnum::EMPTY))
    }

    async fn start_pod(&self, pod: PodObject) -> Result<PodRuntime, DockerError> {
        self.start_pod_calls.lock().await.push(pod.clone());

        if self.fail_start {
            return Err(DockerError::ContainerStartError("Forced error".into()));
        }

        let mut containers_runtime = HashMap::new();

        for container_spec in &pod.spec.containers {
            let container_id = Self::generate_container_id(&container_spec.name);

            self.containers
                .insert(container_id.clone(), ContainerStateStatusEnum::RUNNING);

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
        self.stop_pod_calls.lock().await.push(container_ids.clone());

        if self.fail_stop {
            return Err(DockerError::ContainerStopError("Forced error".into()));
        }

        if self.fail_remove {
            return Err(DockerError::ContainerRemovalError("Forced error".into()));
        }

        for id in container_ids {
            self.containers.remove(id);
        }

        Ok(())
    }

    async fn get_logs(&self, container_id: &str) -> Result<String, DockerError> {
        // Record argument (clone &str to String)
        self.get_logs_calls
            .lock()
            .await
            .push(container_id.to_string());

        Ok("Here, your logs".to_string())
    }

    async fn stream_logs(
        &self,
        id: &str,
    ) -> Result<BoxStream<'static, Result<bytes::Bytes, DockerError>>, DockerError> {
        self.stream_logs_calls.lock().await.push(id.to_string());

        Err(DockerError::StreamLogsError("Forced".into()))
    }
}
