use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::metadata::Metadata;

// --- Core ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pod {
    pub metadata: Metadata,
    pub spec: PodSpec,
    pub status: PodStatus,
}

/// Desired state
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PodSpec {
    pub node_name: String,
    pub containers: Vec<ContainerSpec>,
}

/// Actual state
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PodStatus {
    pub phase: PodPhase,
    pub container_status: Vec<(String, String)>,
    pub last_update: Option<DateTime<Utc>>,
    pub observed_generation: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PodPhase {
    Pending,
    Running,
    Unknown,
    Failed,
    Succeeded,
}

// --- Containers ---

/// Definition of a container within a Pod.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub ports: Option<Vec<Port>>,
    pub env: Option<Vec<EnvVar>>,
}

/// Environment variable for a container.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Port mapping for a container.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Port {
    #[serde(rename = "containerPort")]
    pub container_port: u16,
}

// --- Impl ---

impl Default for PodStatus {
    fn default() -> Self {
        PodStatus {
            phase: PodPhase::Pending,
            container_status: Vec::new(),
            last_update: None,
            observed_generation: 0,
        }
    }
}

impl Default for ContainerSpec {
    fn default() -> Self {
        ContainerSpec {
            name: "test-container".to_string(),
            image: "busybox:latest".to_string(),
            ports: None,
            env: None,
        }
    }
}

impl Default for PodSpec {
    fn default() -> Self {
        PodSpec {
            node_name: "".to_string(),
            containers: vec![ContainerSpec::default()],
        }
    }
}
