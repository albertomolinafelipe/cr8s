use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use shared::models::{
    UserMetadata, Node, PodSpec, PodStatus
};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PodObject {
    id: Uuid,
    node_id: Uuid,
    pod_status: PodStatus,
    metadata: Metadata,
    spec: PodSpec,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
    creation_time: DateTime<Utc>,
    generation: u16,
    modified_at: DateTime<Utc>,
    #[serde(flatten)]
    user: UserMetadata
}

pub struct R8s {
    db: sled::Db
}



impl R8s {
    pub fn new(db: sled::Db) -> Self {
        Self {
            db
        }
    }

    pub fn add_pod(&self, spec: PodSpec, metadata: UserMetadata) -> Uuid {
        let pod = PodObject {
            id: Uuid::new_v4(),
            node_id: Uuid::nil(),
            pod_status: PodStatus::Pending,
            metadata: Metadata::new(metadata),
            spec
        };
        let key = format!("pods/{}", pod.id);
        let value = serde_json::to_vec(&pod).unwrap();
        self.db.insert(key, value).ok();
        pod.id
    }

    pub fn add_node(&self, node: Node) {
        let key = format!("nodes/{}", node.id);
        let value = serde_json::to_vec(&node).unwrap();
        self.db.insert(key, value).ok();
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.db.scan_prefix("nodes/")
            .filter_map(|res| res.ok())
            .filter_map(|(_, val)| serde_json::from_slice::<Node>(&val).ok())
            .collect()
    }
}

impl Metadata {
    pub fn new(user: UserMetadata) -> Self {
        Self { 
            creation_time: Utc::now(),
            generation: 0,
            modified_at: Utc::now(),
            user
        }
    }
}
