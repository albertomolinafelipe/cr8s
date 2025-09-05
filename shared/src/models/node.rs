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

impl Default for Node {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: Uuid::new_v4().to_string(),
            status: NodeStatus::Ready,
            addr: "0.0.0.0:1000".to_string(),
            started_at: Utc::now(),
            last_heartbeat: Utc::now(),
        }
    }
}
