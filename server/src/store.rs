use std::collections::HashSet;

use tokio::sync::broadcast;
use rand::prelude::IteratorRandom;
use dashmap::{DashMap, DashSet};
use uuid::Uuid;

use shared::{
    api::{EventType, PodEvent}, 
    models::{
        Metadata, Node, PodObject, PodSpec, PodStatus, UserMetadata
    }
};

pub enum SpecError {
    WrongFormat(String),
    Conflict(String)
}


pub struct R8s {
    db: sled::Db,
    pod_idx: DashMap<Uuid, DashSet<Uuid>>,
    pub pod_tx: broadcast::Sender<PodEvent>,
}


impl R8s {
    pub fn new(db: sled::Db) -> Self {
        let (pod_tx, _) = broadcast::channel(10);
        Self {
            db,
            pod_idx: DashMap::new(),
            pod_tx
        }
    }

    pub fn add_pod(&self, spec: PodSpec, metadata: UserMetadata) -> Result<Uuid, SpecError> {
        validate_pod(&spec)?;
        let name_key = format!("pod_names/{}", metadata.name);

        if self.db.get(&name_key).ok().flatten().is_some() {
            return Err(SpecError::Conflict("Pod with the same name already exists".to_string()));
        }

        let pod = PodObject {
            id: Uuid::new_v4(),
            node_id: self.optimized_scheduler(),
            pod_status: PodStatus::Pending,
            metadata: Metadata::new(metadata),
            spec,
        };

        let key = format!("pods/{}", pod.id);
        let value = serde_json::to_vec(&pod).unwrap();

        self.db.insert(&key, value).ok();
        self.db.insert(&name_key, pod.id.as_bytes()).ok();

        self.pod_idx.entry(pod.node_id)
            .or_insert_with(DashSet::new)
            .insert(pod.id);

        let event = PodEvent {
            event_type: EventType::ADDED,
            pod: pod.clone(),
        };
        let _ = self.pod_tx.send(event); 

        Ok(pod.id)
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


    pub fn add_node(&self, node: Node) -> Result<(), String> {
        let name_key = format!("node_names/{}", node.name);
        let addr_key = format!("node_addrs/{}", node.addr);

        if self.db.get(&name_key).ok().flatten().is_some() {
            return Err("Node with the same name already exists".to_string());
        }

        if self.db.get(&addr_key).ok().flatten().is_some() {
            return Err("Node with the same address already exists".to_string());
        }

        let key = format!("nodes/{}", node.id);
        let value = serde_json::to_vec(&node).unwrap();

        self.db.insert(&key, value).ok();
        self.db.insert(&name_key, node.id.as_bytes()).ok();
        self.db.insert(&addr_key, node.id.as_bytes()).ok();
        self.pod_idx.entry(node.id).or_insert_with(DashSet::new);

        Ok(())
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


fn validate_pod(spec: &PodSpec) -> Result<(), SpecError> {
    let mut seen_names = HashSet::new();

    for container in &spec.containers {
        if !seen_names.insert(&container.name) {
            return Err(SpecError::WrongFormat(format!(
                "Duplicate container name found: '{}'",
                container.name
            )));
        }
    }

    Ok(())
}
