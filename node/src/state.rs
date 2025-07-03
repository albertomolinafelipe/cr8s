use actix_web::web;
use bollard::secret::ContainerStateStatusEnum;
use dashmap::{DashMap, DashSet};
use shared::models::PodObject;
use std::env;
use std::sync::RwLock;
use uuid::Uuid;

use crate::docker::DockerManager;

pub type State = web::Data<NodeState>;

#[derive(Debug, Clone)]
pub struct PodRuntime {
    pub id: Uuid,
    pub containers: Vec<ContainerRuntime>,
}

#[derive(Debug, Clone)]
pub struct ContainerRuntime {
    pub id: String,
    pub name: String,
    pub status: ContainerStateStatusEnum,
}

#[derive(Debug)]
pub struct NodeState {
    pub config: Config,
    pub docker_mgr: DockerManager,
    node_name: RwLock<String>,
    pods: DashMap<Uuid, PodObject>,
    pod_runtimes: DashMap<Uuid, PodRuntime>,
    pod_names: DashSet<String>,
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            config: load_config(),
            docker_mgr: DockerManager::new(),
            node_name: RwLock::new(String::new()),
            pods: DashMap::new(),
            pod_runtimes: DashMap::new(),
            pod_names: DashSet::new(),
        }
    }

    pub fn node_name(&self) -> String {
        self.node_name.read().unwrap().clone()
    }

    pub fn set_name(&self, name: String) {
        *self.node_name.write().unwrap() = name;
    }

    pub fn get_pod(&self, id: &Uuid) -> Option<PodObject> {
        self.pods.get(id).map(|r| r.clone())
    }

    pub fn get_pod_runtime(&self, id: &Uuid) -> Option<PodRuntime> {
        self.pod_runtimes.get(id).map(|r| r.clone())
    }

    pub fn get_pod_names(&self) -> Vec<String> {
        self.pods
            .iter()
            .map(|p| p.metadata.user.name.clone())
            .collect()
    }

    pub fn add_pod(&self, pod: &PodObject) -> Result<(), String> {
        if self.pods.contains_key(&pod.id) {
            return Err(format!("Pod with ID '{}' already exists.", pod.id));
        }

        if self.pod_names.contains(&pod.metadata.user.name) {
            return Err(format!(
                "Pod with name '{}' already exists.",
                pod.metadata.user.name
            ));
        }

        self.pods.insert(pod.id, pod.clone());
        self.pod_names.insert(pod.metadata.user.name.clone());

        Ok(())
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
}

#[derive(Debug)]
pub struct Config {
    pub server_url: String,
    pub port: u16,
    pub name: String,
    pub register_retries: u16,
    pub node_api_workers: usize,
    pub sync_loop: u16,
}

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
        .unwrap_or(10);

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
