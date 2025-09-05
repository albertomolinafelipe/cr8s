use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    Running,
    Stopped,
}
