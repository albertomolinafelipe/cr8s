use rand::prelude::IteratorRandom;
use dashmap::{DashMap, DashSet};
use uuid::Uuid;

use shared::models::{
    UserMetadata, Node, PodSpec, PodStatus, PodObject, Metadata
};


pub struct R8s {
    db: sled::Db,
    pod_idx: DashMap<Uuid, DashSet<Uuid>>
}


impl R8s {
    pub fn new(db: sled::Db) -> Self {
        Self {
            db,
            pod_idx: DashMap::new()
        }
    }

    pub fn add_pod(&self, spec: PodSpec, metadata: UserMetadata) -> Uuid {
        let pod = PodObject {
            id: Uuid::new_v4(),
            node_id: self.optimized_scheduler(),
            pod_status: PodStatus::Pending,
            metadata: Metadata::new(metadata),
            spec
        };
        let key = format!("pods/{}", pod.id);
        let value = serde_json::to_vec(&pod).unwrap();
        
        // Insert into store and index
        self.db.insert(key, value).ok();
        self.pod_idx.entry(pod.node_id)
            .or_insert_with(DashSet::new)
            .insert(pod.id);
        pod.id
    }

    pub fn get_pods(&self, query: Option<Uuid>) -> Vec<PodObject> {
        match query {
            Some(node_id) => {
                self.pod_idx.get(&node_id)
                    .map(|set| {
                        set.iter()
                            .filter_map(|pod_id| {
                                let key = format!("pods/{}", *pod_id);
                                self.db.get(key).ok().flatten()
                                    .and_then(|val| serde_json::from_slice::<PodObject>(&val).ok())
                            })
                        .collect()
                    })
                .unwrap_or_default()
            }
            None => {
                self.db.scan_prefix("pods/")
                    .filter_map(|res| res.ok())
                    .filter_map(|(_, val)| serde_json::from_slice::<PodObject>(&val).ok())
                    .collect()
            }
        }
    }


    pub fn add_node(&self, node: Node) {
        let key = format!("nodes/{}", node.id);
        let value = serde_json::to_vec(&node).unwrap();
        self.db.insert(key, value).ok();
        self.pod_idx.entry(node.id).or_insert_with(DashSet::new);
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.db.scan_prefix("nodes/")
            .filter_map(|res| res.ok())
            .filter_map(|(_, val)| serde_json::from_slice::<Node>(&val).ok())
            .collect()
    }

    fn optimized_scheduler(&self) -> Uuid {
        let mut rng = rand::rng();
        self.pod_idx.iter()
            .map(|entry| *entry.key())
            .choose(&mut rng)
            .unwrap_or_else(Uuid::nil)
    }
}
