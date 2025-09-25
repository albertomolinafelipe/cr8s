use serde::{Deserialize, Serialize};

use crate::{api::PodManifest, models::metadata::Metadata};

// --- Core ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicaSet {
    pub metadata: Metadata,
    pub spec: ReplicaSetSpec,
    pub status: ReplicaSetStatus,
}

/// Actual state
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicaSetStatus {
    pub ready_replicas: u16,
    pub observed_generation: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicaSetSpec {
    pub replicas: u16,
    pub template: PodManifest,
}

// --- Impl ---

impl Default for ReplicaSetStatus {
    fn default() -> Self {
        ReplicaSetStatus {
            ready_replicas: 0,
            observed_generation: 0,
        }
    }
}
