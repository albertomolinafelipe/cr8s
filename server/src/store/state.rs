use actix_web::web::Data;
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use futures::future::join_all;
use std::collections::HashSet;
use tokio::sync::broadcast;
use uuid::Uuid;

use shared::{
    api::{EventType, NodeEvent, PodEvent},
    models::{Metadata, Node, PodObject, PodSpec, PodStatus, UserMetadata},
};

use crate::State;

use super::{
    errors::StoreError,
    store::{EtcdStore, Store},
};

pub async fn new_state() -> State {
    let r8s = R8s::new().await;
    Data::new(r8s)
}

#[cfg(test)]
pub async fn new_state_with_store(store: Box<dyn Store + Send + Sync>) -> State {
    let r8s = R8s::default_with_store(store).await;
    Data::new(r8s)
}

pub struct R8s {
    store: Box<dyn Store + Send + Sync>,
    pub pod_tx: broadcast::Sender<PodEvent>,
    pub node_tx: broadcast::Sender<NodeEvent>,

    pub node_names: DashSet<String>,
    node_addrs: DashSet<String>,
    /// Assigned pods per node
    pub pod_map: DashMap<String, DashSet<Uuid>>,
    pub pod_name_idx: DashMap<String, Uuid>,
}

impl R8s {
    pub async fn new() -> Self {
        Self::default_with_store(Box::new(EtcdStore::new().await)).await
    }

    async fn default_with_store(store: Box<dyn Store + Send + Sync>) -> Self {
        let (pod_tx, _) = broadcast::channel(10);
        let (node_tx, _) = broadcast::channel(10);
        Self {
            store,
            pod_tx,
            node_tx,
            node_names: DashSet::new(),
            node_addrs: DashSet::new(),
            pod_map: DashMap::new(),
            pod_name_idx: DashMap::new(),
        }
    }

    pub async fn add_pod(&self, spec: PodSpec, user: UserMetadata) -> Result<Uuid, StoreError> {
        // validate spec and name
        validate_pod(&spec)?;
        (!self.pod_name_idx.contains_key(&user.name))
            .then_some(())
            .ok_or_else(|| StoreError::Conflict("Duplicate pod name".to_string()))?;

        // Since its low level object manifest is not stored
        let pod = PodObject {
            metadata: Metadata {
                user,
                ..Default::default()
            },
            spec,
            ..Default::default()
        };
        self.store.put_pod(&pod.id, &pod).await?;
        self.pod_name_idx
            .insert(pod.metadata.user.name.clone(), pod.id);
        self.pod_map
            .entry("".to_string())
            .or_default()
            .insert(pod.id);

        let event = PodEvent {
            event_type: EventType::Added,
            pod: pod.clone(),
        };
        let _ = self.pod_tx.send(event);
        Ok(pod.id)
    }

    pub async fn assign_pod(&self, name: &str, node_name: String) -> Result<(), StoreError> {
        // Check node name exists
        (self.node_names.contains(&node_name))
            .then_some(())
            .ok_or_else(|| {
                StoreError::InvalidReference(format!("No node exists with name={}", node_name))
            })?;

        // Check pod name exists and its in the unassigned set
        let Some(pod_id) = self.pod_name_idx.get(name) else {
            return Err(StoreError::NotFound(format!(
                "No pod exists with name={}",
                name
            )));
        };

        let Some(unassigned_entry) = self.pod_map.get("") else {
            return Err(StoreError::Conflict("Unassigned set not found".to_string()));
        };

        // Check pod is unassigned
        let mut pod = self
            .store
            .get_pod(pod_id.clone())
            .await?
            .ok_or(StoreError::NotFound("Pod not found in store".to_string()))?;
        if !pod.node_name.is_empty() {
            return Err(StoreError::Conflict(format!(
                "Pod ({}) is already assigned to a node",
                name
            )));
        }

        // Assign and insert pod
        pod.node_name = node_name.clone();
        self.store.put_pod(&pod.id, &pod).await?;

        // Update indeces
        unassigned_entry.remove(&*pod_id);
        self.pod_map
            .entry(node_name)
            .or_insert_with(DashSet::new)
            .insert(*pod_id);
        let event = PodEvent {
            event_type: EventType::Modified,
            pod: pod.clone(),
        };
        let _ = self.pod_tx.send(event);
        Ok(())
    }

    pub async fn update_pod_status(
        &self,
        id: Uuid,
        status: PodStatus,
        container_statuses: &mut Vec<(String, String)>,
    ) -> Result<(), StoreError> {
        let mut pod = self
            .store
            .get_pod(id)
            .await?
            .ok_or(StoreError::NotFound("Node not found in store".to_string()))?;
        validate_container_statuses(&pod.spec, container_statuses);
        pod.last_status_update = Some(Utc::now());
        pod.pod_status = status;
        pod.container_status = container_statuses.clone();
        self.store.put_pod(&id, &pod).await?;
        Ok(())
    }

    pub async fn get_pods(&self, query: Option<String>) -> Vec<PodObject> {
        match query {
            Some(node_name) => {
                let Some(pod_ids_ref) = self.pod_map.get(&node_name) else {
                    return vec![];
                };
                join_all(pod_ids_ref.iter().map(|id| self.store.get_pod(id.clone())))
                    .await
                    .into_iter()
                    .inspect(|res| {
                        if let Err(e) = res {
                            tracing::error!(error=%e, "Error fetching pod");
                        }
                    })
                    .filter_map(Result::ok)
                    .flatten()
                    .collect()
            }
            None => self.store.list_pods().await.unwrap_or_default(),
        }
    }

    pub async fn add_node(&self, node: &Node) -> Result<(), StoreError> {
        (!node.name.is_empty())
            .then_some(())
            .ok_or_else(|| StoreError::WrongFormat("Node name is empty".to_string()))?;

        (!self.node_addrs.contains(&node.addr) && !self.node_names.contains(&node.name))
            .then_some(())
            .ok_or_else(|| StoreError::Conflict("Duplicate node name or address".to_string()))?;

        // Into store
        self.node_addrs.insert(node.addr.clone());
        self.node_names.insert(node.name.clone());

        let event = NodeEvent {
            event_type: EventType::Added,
            node: node.clone(),
        };
        let _ = self.node_tx.send(event);
        Ok(())
    }

    pub async fn get_nodes(&self) -> Vec<Node> {
        self.store.list_nodes().await.unwrap_or_default()
    }

    pub async fn update_node_heartbeat(&self, node_name: &str) -> Result<(), StoreError> {
        let mut node = self
            .store
            .get_node(node_name)
            .await?
            .ok_or(StoreError::NotFound("Node not found in store".to_string()))?;

        node.last_heartbeat = Utc::now();
        self.store.put_node(node_name, &node).await
    }
}

/// Check for duplicate container name in spec
fn validate_pod(spec: &PodSpec) -> Result<(), StoreError> {
    let mut seen_names = HashSet::new();

    for container in &spec.containers {
        if !seen_names.insert(&container.name) {
            return Err(StoreError::WrongFormat(format!(
                "Duplicate container name found: '{}'",
                container.name
            )));
        }
    }

    Ok(())
}

fn validate_container_statuses(spec: &PodSpec, container_statuses: &mut Vec<(String, String)>) {
    let valid_names: HashSet<_> = spec.containers.iter().map(|c| c.name.clone()).collect();

    // Filter out any entries invalid names, ignore extra names
    container_statuses.retain(|(name, _)| valid_names.contains(name));

    let existing_names: HashSet<_> = container_statuses
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    for container in &spec.containers {
        if !existing_names.contains(&container.name) {
            container_statuses.push((container.name.clone(), "EMPTY".to_string()));
        }
    }
}
