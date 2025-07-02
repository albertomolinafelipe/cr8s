use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::{Arc, RwLock};
use actix_web::web;
use bollard::secret::ContainerStateStatusEnum;
use bollard::Docker;
use shared::models::PodObject;
use uuid::Uuid;

use crate::docker::docker_client;


pub type State = web::Data<NodeState>;

#[derive(Debug, Clone)]
pub struct PodRuntime {
    pub id: Uuid,
    pub containers: Vec<ContainerRuntime>
}

#[derive(Debug, Clone)]
pub struct ContainerRuntime {
    pub id: String,
    pub status: ContainerStateStatusEnum,
}

#[derive(Debug)]
pub struct NodeState {
    pub config: Config,
    client: Arc<Docker>,
    node_name: RwLock<String>,
    pods: RwLock<HashMap<Uuid, PodObject>>,
    pod_runtimes: RwLock<HashMap<Uuid, PodRuntime>>,
    pod_names: RwLock<HashSet<String>>,
    images: RwLock<HashSet<String>>
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            config: load_config(),
            client: docker_client(),
            node_name: RwLock::new(String::new()),
            pods: RwLock::new(HashMap::new()),
            pod_runtimes: RwLock::new(HashMap::new()),
            pod_names: RwLock::new(HashSet::new()),
            images: RwLock::new(HashSet::new()),
        }
    }

    pub fn has_image(&self, image: &str) -> bool {
        self.images.read().unwrap().contains(image)
    }

    pub fn mark_image_as_pulled(&self, image: String) {
        self.images.write().unwrap().insert(image);
    }

    pub fn docker_client(&self) -> Arc<Docker> {
        self.client.clone()
    }

    pub fn node_name(&self) -> String {
        self.node_name.read().unwrap().clone()
    }

    pub fn set_name(&self, name: String) {
        *self.node_name.write().unwrap() = name;
    }

    pub fn get_pod(&self, id: &Uuid) -> Option<PodObject>{
        self.pods.read().unwrap().get(id).cloned()
    }

    pub fn get_pod_runtime(&self, id: &Uuid) -> Option<PodRuntime>{
        self.pod_runtimes.read().unwrap().get(id).cloned()
    }

    pub fn get_pod_names(&self) -> Vec<String> {
        self.pods
            .read()
            .unwrap()
            .values()
            .map(|p| p.metadata.user.name.clone())
            .collect()
    }

    pub fn add_pod(&self, pod: &PodObject) -> Result<(), String> {
        let mut pods = self.pods.write().unwrap();
        let mut pod_names = self.pod_names.write().unwrap();

        if pods.contains_key(&pod.id) {
            return Err(format!("Pod with ID '{}' already exists.", pod.id));
        }

        if pod_names.contains(&pod.metadata.user.name) {
            return Err(format!("Pod with name '{}' already exists.", pod.metadata.user.name));
        }

        pods.insert(pod.id, pod.clone());
        pod_names.insert(pod.metadata.user.name.clone());

        Ok(())
    }

    pub fn add_pod_runtime(&self, pod_runtime: PodRuntime) -> Result<(), String> {
        let mut runtimes = self.pod_runtimes.write().unwrap();
        if runtimes.contains_key(&pod_runtime.id) {
            return Err(format!("PodRuntime with ID '{}' already exists.", pod_runtime.id));
        }
        runtimes.insert(pod_runtime.id, pod_runtime);
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
}

fn load_config() -> Config {
    let server_address = env::var("R8S_SERVER_HOST")
        .unwrap_or_else(|_| "localhost".to_string());
    
    let server_port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(7620);

    let port = env::var("NODE_PORT")
        .expect("NODE_PORT environment variable is required")
        .parse()
        .expect("NODE_PORT must be a valid number");
    
    let name = env::var("NODE_NAME")
        .unwrap_or_else(|_| format!("worker-node-{}", port));

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
        register_retries,
        node_api_workers,
    }
}
