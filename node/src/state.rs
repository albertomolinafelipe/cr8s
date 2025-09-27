//! # Node State Management
//!
//! This module defines the in-memory state of a node in the cluster
//! Including its config, known pods, runtime container info and docker

use std::collections::HashMap;

use actix_web::web::Data;
use bollard::secret::ContainerStateStatusEnum;
use dashmap::DashMap;
use shared::models::pod::{Pod, PodPhase};
use uuid::Uuid;

use crate::{
    docker::{DockerClient, DockerManager},
    models::{Config, PodRuntime},
};

/// Thread safe wrapper
pub type State = Data<NodeState>;

/// Global in-memory state for a single node.
pub struct NodeState {
    pub config: Config,
    pub docker_mgr: Box<dyn DockerClient + Send + Sync>,
    pods: DashMap<Uuid, Pod>,
    pod_runtimes: DashMap<Uuid, PodRuntime>,
}

impl NodeState {
    /// Initializes a new [`NodeState`] instance, loading config and starting Docker manager.
    pub fn new_with(
        config_in: Option<Config>,
        docker_in: Option<Box<dyn DockerClient + Send + Sync>>,
    ) -> State {
        let docker_mgr = docker_in.unwrap_or_else(|| {
            Box::new(
                DockerManager::start()
                    .inspect_err(
                        |err| tracing::error!(error = %err, "Failed to start docker manager"),
                    )
                    .expect("Docker manager failed to start"),
            )
        });

        let config = config_in.unwrap_or_else(Config::from_env);
        Data::new(Self {
            config,
            docker_mgr,
            pods: DashMap::new(),
            pod_runtimes: DashMap::new(),
        })
    }
    pub fn new() -> State {
        Self::new_with(None, None)
    }

    // --- Pods ---

    pub fn get_pod(&self, id: &Uuid) -> Option<Pod> {
        self.pods.get(id).map(|r| r.clone())
    }
    pub fn put_pod(&self, pod: &Pod) {
        self.pods.insert(pod.metadata.id, pod.clone());
    }
    pub fn delete_pod(&self, id: &Uuid) {
        self.pods.remove(id);
    }

    // --- Pod Runtimes ---

    pub fn get_pod_runtime(&self, id: &Uuid) -> Option<PodRuntime> {
        self.pod_runtimes.get(id).map(|r| r.clone())
    }

    pub fn list_pod_runtimes(&self) -> Vec<PodRuntime> {
        self.pod_runtimes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
    pub fn delete_pod_runtime(&self, id: &Uuid) {
        self.pod_runtimes.remove(id);
    }
    pub fn add_pod_runtime(&self, pod_runtime: PodRuntime) -> Result<(), String> {
        if self.pod_runtimes.contains_key(&pod_runtime.id) {
            return Err(format!(
                "PodRuntime with ID '{}' already exists.",
                pod_runtime.id
            ));
        }
        self.pod_runtimes.insert(pod_runtime.id, pod_runtime);
        Ok(())
    }

    /// Updates the runtime status of a pod by merging new container statuses
    /// Get aggregate pod status, simplified
    pub fn update_pod_runtime_status(
        &self,
        pod_id: &Uuid,
        container_statuses: HashMap<String, ContainerStateStatusEnum>,
    ) -> Result<PodPhase, String> {
        if let Some(mut pod_runtime) = self.pod_runtimes.get_mut(pod_id) {
            // Update each container status in pod_runtime
            let mut pod_status = PodPhase::Running;
            for (spec_name, status) in container_statuses {
                if let Some(container) = pod_runtime.containers.get_mut(&spec_name) {
                    container.status = status.clone();
                    if container.status != ContainerStateStatusEnum::RUNNING {
                        pod_status = PodPhase::Succeeded;
                    }
                }
            }
            Ok(pod_status)
        } else {
            Err(format!("PodRuntime with ID '{}' not found", pod_id))
        }
    }
}
