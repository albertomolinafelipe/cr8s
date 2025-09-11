use std::{collections::HashMap, env};

use bollard::secret::ContainerStateStatusEnum;
use serde::{Deserialize, Serialize};
use shared::api::EventType;
use uuid::Uuid;

// --- State objects ---

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

// --- Thread communication ---

// Information passed through the channels
pub struct WorkRequest {
    pub id: Uuid,
    pub event: EventType,
}

// --- Config definition ---

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

impl Config {
    /// Loads node configuration from environment variables.
    ///
    /// Falls back to defaults when applicable.
    pub fn from_env() -> Self {
        let mut config = Config::default();

        if let Ok(addr) = env::var("R8S_SERVER_HOST") {
            let port = env::var("R8S_SERVER_PORT")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(7620);
            config.server_url = format!("http://{}:{}", addr, port);
        }

        if let Some(p) = env::var("NODE_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            config.port = p;
        }

        config.name = env::var("NODE_NAME").unwrap_or_else(|_| {
            let uuid = Uuid::new_v4().to_string();
            format!("r8sagt-{}", &uuid[..4])
        });

        if let Some(val) = env::var("SYNC_LOOP_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            config.sync_loop = val;
        }

        if let Some(val) = env::var("NODE_REGISTER_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            config.register_retries = val;
        }

        if let Some(val) = env::var("NODE_API_WORKERS")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            config.node_api_workers = val;
        }

        config
    }
}

impl Default for Config {
    fn default() -> Self {
        let port = 7621;
        Self {
            server_url: "http://localhost:7620".to_string(),
            port,
            name: format!("worker-node-{}", port),
            sync_loop: 15,
            register_retries: 3,
            node_api_workers: 2,
        }
    }
}
