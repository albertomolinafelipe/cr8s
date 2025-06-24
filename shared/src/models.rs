use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PodObject {
    pub id: Uuid,
    pub node_id: Uuid,
    pub pod_status: PodStatus,
    pub metadata: Metadata,
    pub spec: PodSpec,
}

/// Specification of a Pod
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PodSpec {
    pub containers: Vec<ContainerSpec>,
}

/// Definition of a container within a Pod.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    pub ports: Vec<Port>,
    pub env: Vec<EnvVar>,
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

/// Metadata for any top-level object, includes at least a name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserMetadata {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
    pub created_at: DateTime<Utc>,
    generation: u16,
    modified_at: DateTime<Utc>,
    #[serde(flatten)]
    pub user: UserMetadata
}

/// Represents a node in the cluster.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub status: NodeStatus,
    pub addr: String,
    pub started_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

/// Status of a node in the cluster.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum NodeStatus {
    Ready,
    Stopped,
}

/// Status of a Pod during its lifecycle.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum PodStatus {
    Pending,
    Running,
    Failed,
    Succeeded,
    Unknown,
}


impl Metadata {
    pub fn new(user: UserMetadata) -> Self {
        Self { 
            created_at: Utc::now(),
            generation: 0,
            modified_at: Utc::now(),
            user
        }
    }
}

impl ContainerSpec {
    pub fn new() -> Self {
        Self { 
            name: "name".to_string(), 
            image: "image".to_string(), 
            ports: Vec::new(), 
            env:  Vec::new(),
        }
    }
}
