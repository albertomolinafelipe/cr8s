//! # Node State Management
//!
//! This module defines the in-memory state of a node in the cluster
//! Including its config, known pods, runtime container info and docker

use actix_web::web::Data;
use dashmap::DashMap;
use shared::models::PodObject;
use uuid::Uuid;

use crate::{
    docker::manager::{DockerClient, DockerManager},
    models::{Config, PodRuntime},
};

/// Thread safe wrapper
pub type State = Data<NodeState>;

pub async fn new_state() -> State {
    let docker_mgr = Box::new(
        DockerManager::start()
            .inspect_err(|err| tracing::error!(error=%err, "Failed to start docker manager"))
            .expect(""),
    );
    Data::new(NodeState::default_with_docker(docker_mgr).await)
}

#[cfg(test)]
pub async fn new_state_with_docker(docker: Box<dyn DockerClient + Send + Sync>) -> State {
    Data::new(NodeState::default_with_docker(docker).await)
}

/// Global in-memory state for a single node.
pub struct NodeState {
    pub config: Config,
    pub docker_mgr: Box<dyn DockerClient + Send + Sync>,
    pods: DashMap<Uuid, PodObject>,
    pod_runtimes: DashMap<Uuid, PodRuntime>,
}

impl NodeState {
    /// Initializes a new [`NodeState`] instance, loading config and starting Docker manager.
    async fn default_with_docker(docker: Box<dyn DockerClient + Send + Sync>) -> Self {
        Self {
            config: Config::from_env(),
            docker_mgr: docker,
            pods: DashMap::new(),
            pod_runtimes: DashMap::new(),
        }
    }

    /// Returns a pod by ID if it exists in the local pod cache.
    pub fn get_pod(&self, id: &Uuid) -> Option<PodObject> {
        self.pods.get(id).map(|r| r.clone())
    }

    /// Returns the runtime info of a pod by ID if available.
    pub fn get_pod_runtime(&self, id: &Uuid) -> Option<PodRuntime> {
        self.pod_runtimes.get(id).map(|r| r.clone())
    }

    /// Returns all tracked pod runtime entries.
    pub fn list_pod_runtimes(&self) -> Vec<PodRuntime> {
        self.pod_runtimes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Inserts or updates a pod definition in the cache.
    pub fn put_pod(&self, pod: &PodObject) {
        self.pods.insert(pod.id, pod.clone());
    }

    /// Removes a pod from the pod cache.
    pub fn delete_pod(&self, id: &Uuid) {
        self.pods.remove(id);
    }

    /// Removes a pod runtime entry from the runtime cache.
    pub fn delete_pod_runtime(&self, id: &Uuid) {
        self.pod_runtimes.remove(id);
    }

    /// Adds a new pod runtime entry if it doesn't already exist.
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
}
