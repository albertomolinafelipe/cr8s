use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{PodContainers, PodManifest},
    models::metadata::{Metadata, ObjectMetadata, OwnerKind, OwnerReference},
};

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

impl From<ReplicaSet> for PodManifest {
    fn from(rs: ReplicaSet) -> Self {
        let short = &Uuid::new_v4().to_string()[..4];
        Self {
            metadata: ObjectMetadata {
                name: format!("{}-{}", rs.metadata.name, short),
                owner_reference: Some(OwnerReference {
                    id: rs.metadata.id,
                    name: rs.metadata.name.clone(),
                    kind: OwnerKind::ReplicaSet,
                    controller: true,
                }),
            },
            spec: PodContainers {
                containers: rs.spec.template.spec.containers,
            },
        }
    }
}
