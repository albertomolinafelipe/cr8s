use std::sync::RwLock;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use shared::models::{
    Metadata, Node, PodSpec, PodStatus, SpecObject, Spec
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
enum StateObject {
    Pod {
        id: Uuid,
        node_id: Uuid,
        pod_status: PodStatus,
        metadata: Metadata,
        spec: PodSpec,
    },
}


#[derive(Debug, Deserialize, Serialize)]
struct DesiredState {
    version: u16,
    modified_at: RwLock<Option<DateTime<Utc>>>,
    objects: RwLock<Vec<StateObject>>
}

#[derive(Debug, Deserialize, Serialize)]
struct ClusterState {
    objects: Vec<StateObject>,
    nodes: RwLock<Vec<Node>>,
}

pub struct R8s {
    cluster_state: ClusterState,
    desired_state: DesiredState,
    spec: RwLock<Vec<SpecObject>>
}



impl R8s {
    pub fn new() -> Self {
        Self {
            cluster_state: ClusterState { 
                nodes: RwLock::new(Vec::new()),
                objects: Vec::new()
            },
            desired_state: DesiredState { 
                version: 0, 
                modified_at: RwLock::new(None),
                objects: RwLock::new(Vec::new()) 
            },
            spec: RwLock::new(Vec::new()),
        }
    }

    pub fn add_object(&self, obj: SpecObject) {
        // add spec
        self.spec.write().unwrap().push(obj.clone());
        
        // scheduling
        let spec = match obj.spec {
            Spec::Pod(ref spec) => spec,
        };
        let state_object = StateObject::Pod { 
            id: Uuid::new_v4(), 
            node_id: Uuid::nil(), 
            pod_status: PodStatus::Pending, 
            metadata: obj.metadata,
            spec: spec.clone()
        };
        *self.desired_state.modified_at.write().unwrap() = Some(Utc::now());
        self.desired_state.objects.write().unwrap().push(state_object);
    }

    pub fn add_node(&self, node: Node) {
        self.cluster_state.nodes.write().unwrap().push(node);
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.cluster_state.nodes.read().unwrap().clone()
    }
}

