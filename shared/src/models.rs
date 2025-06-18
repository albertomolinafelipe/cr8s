use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tabled::Tabled;
use uuid::Uuid;
use std::borrow::Cow;

/// Represents a top-level Kubernetes-like object with metadata and a kind-specific spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpecObject {
    pub metadata: Metadata,
    
    #[serde(flatten)]
    pub spec: Spec,
}

/// Enum representing the specification of an object based on its kind.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "spec", rename_all = "PascalCase")]
pub enum Spec {
    Pod(PodSpec),
}

/// Enum for supported object kinds.
#[derive(Debug, Clone, Deserialize, Serialize)]
enum Kind {
    Pod,
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
pub struct Metadata {
    pub name: String,
}

/// Represents a node in the cluster.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub status: NodeStatus,
    pub api_url: String,
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PodStatus {
    Pending,
    Running,
    Failed,
    Unknown,
}


impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Ready => write!(f, "Ready"),
            NodeStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

impl std::fmt::Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Spec::Pod(_) => write!(f, "pod"),
        }
    }
}



impl Tabled for Node {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        vec![
            Cow::Owned(self.name.clone()),
            Cow::Owned(self.status.to_string()),
            Cow::Owned(self.api_url.clone()),
            Cow::Owned(human_duration(Utc::now().signed_duration_since(self.started_at).to_std().unwrap_or_default())),
        ]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec![
            Cow::Borrowed("NAME"),
            Cow::Borrowed("STATUS"),
            Cow::Borrowed("ADDRESS"),
            Cow::Borrowed("AGE"),
        ]
    }
}


fn human_duration(dur: std::time::Duration) -> String {
    let secs = dur.as_secs();
    match secs {
        0..=59 => format!("{}s ago", secs),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}
