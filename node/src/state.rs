//! # Node State Management
//!
//! This module defines the in-memory state of a node in the cluster
//! Including its confi, known pods, runtime container info and docker

use actix_web::web;
use bollard::secret::ContainerStateStatusEnum;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use shared::models::PodObject;
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

use crate::docker::manager::{DockerClient, DockerManager};

/// Thread safe wrapper
pub type State = web::Data<NodeState>;

/// Runtime information for a pod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodRuntime {
    pub id: Uuid,
    pub name: String,
    pub containers: HashMap<String, ContainerRuntime>,
}

/// Runtime information for a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRuntime {
    pub id: String,
    pub spec_name: String,
    pub name: String,
    pub status: ContainerStateStatusEnum,
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
    pub fn new() -> Self {
        let docker_mgr = Box::new(
            DockerManager::start()
                .inspect_err(|err| tracing::error!(error=%err, "Failed to start docker manager"))
                .expect(""),
        );
        Self {
            config: load_config(),
            docker_mgr,
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

    /// Returns a list of all pod names currently registered.
    pub fn get_pod_names(&self) -> Vec<String> {
        self.pods
            .iter()
            .map(|p| p.metadata.user.name.clone())
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

    /// Updates the runtime status of a pod by merging new container statuses.
    pub fn update_pod_runtime_status(
        &self,
        pod_id: &Uuid,
        container_statuses: HashMap<String, ContainerStateStatusEnum>,
    ) -> Result<(), String> {
        if let Some(mut pod_runtime) = self.pod_runtimes.get_mut(pod_id) {
            // Update each container status in pod_runtime
            for (spec_name, status) in container_statuses {
                if let Some(container) = pod_runtime.containers.get_mut(&spec_name) {
                    container.status = status.clone();
                }
            }
            Ok(())
        } else {
            Err(format!("PodRuntime with ID '{}' not found", pod_id))
        }
    }
}

/// Node configuration loaded from environment variables.
#[derive(Debug)]
pub struct Config {
    pub server_url: String,
    pub port: u16,
    pub name: String,
    pub register_retries: u16,
    pub node_api_workers: usize,
    pub sync_loop: u16,
}

/// Loads node configuration from environment variables.
///
/// Falls back to defaults when applicable.
/// Panics if `NODE_PORT` is missing or invalid.
fn load_config() -> Config {
    let server_address = env::var("R8S_SERVER_HOST").unwrap_or_else(|_| "localhost".to_string());

    let server_port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(7620);

    let port = env::var("NODE_PORT")
        .expect("NODE_PORT environment variable is required")
        .parse()
        .expect("NODE_PORT must be a valid number");

    let sync_loop = env::var("SYNC_LOOP_INTERVAL")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(15);

    let name = env::var("NODE_NAME").unwrap_or_else(|_| format!("worker-node-{}", port));

    let register_retries = env::var("NODE_REGISTER_RETRIES")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3);

    let node_api_workers = env::var("NODE_API_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2);

    Config {
        server_url: format!("http://{}:{}", server_address, server_port),
        port,
        name,
        sync_loop,
        register_retries,
        node_api_workers,
    }
}
