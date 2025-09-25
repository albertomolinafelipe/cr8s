//! State management for apiserver
//!
//! Manage pods and nodes
//! Provides abstraction for persistent storage and caching layer
//! Event broadcasting mechanism for notifications on watches

mod cache;
mod errors;
mod store;
#[cfg(test)]
pub mod test_store;

use actix_web::web;
use chrono::Utc;
use futures::future::join_all;
use std::collections::HashSet;
use tokio::sync::broadcast;
use uuid::Uuid;

use shared::{
    api::{EventType, NodeEvent, PodEvent, ReplicaSetEvent},
    models::{
        metadata::Metadata,
        node::Node,
        pod::{ContainerSpec, Pod, PodSpec, PodStatus},
        replicaset::{ReplicaSet, ReplicaSetSpec, ReplicaSetStatus},
    },
};

use cache::CacheManager;
use errors::StoreError;
use store::{EtcdStore, Store};

pub type State = web::Data<ApiServerState>;

/// Core with storage, caches, and event channels.
pub struct ApiServerState {
    store: Box<dyn Store + Send + Sync>,
    /// Broadcast channels
    pub pod_tx: broadcast::Sender<PodEvent>,
    pub node_tx: broadcast::Sender<NodeEvent>,
    pub replicaset_tx: broadcast::Sender<ReplicaSetEvent>,
    /// In-memory fast-access cache for node/pod metadata.
    pub cache: CacheManager,
}

impl ApiServerState {
    //! - add_pod(spec, metadata): Validate and add a new pod to the store and cache, then broadcast an event
    //! - delete_pod(name): Remove a pod the store and cache, then broadcast an event
    //! - assign_pod(name, node_name): Assign an unassigned pod to a  ode, update store and cache, broadcast event
    //! - update_pod_status(id, status, cont_status): Update the status and container statuses of a pod
    //! - get_pods(query): List pods optionally filtered by node name
    //!
    //! - add_replicaset(sepc, metadata)
    //! - get_replicasets()
    //!
    //! - add_node(node): Add a new node to the store and cache, then broadcast an event
    //! - get_nodes(): Retrieve all Nodes from the store
    //! - get_node(name): Get a specific Node by name from the store
    //! - update_node_heartbeat(node_name): Update the heartbeat timestamp of a node in the store

    /// Construc ts a new instance with a custom store implementation.

    pub async fn new() -> State {
        Self::new_with_store(Box::new(EtcdStore::new().await)).await
    }

    pub async fn new_with_store(store: Box<dyn Store + Send + Sync>) -> State {
        let (pod_tx, _) = broadcast::channel(10);
        let (node_tx, _) = broadcast::channel(10);
        let (replicaset_tx, _) = broadcast::channel(10);
        let cache = CacheManager::new();
        web::Data::new(Self {
            store,
            pod_tx,
            node_tx,
            replicaset_tx,
            cache,
        })
    }

    pub async fn add_replicaset(
        &self,
        spec: ReplicaSetSpec,
        metadata: Metadata,
    ) -> Result<Uuid, StoreError> {
        validate_container_list(&spec.template.spec.containers)?;

        // save object and metadata in store and cache
        let rs = ReplicaSet {
            spec,
            metadata,
            status: ReplicaSetStatus::default(),
        };

        self.store.put_replicaset(&rs.metadata.id, &rs).await?;
        self.cache.add_replicaset(&rs.metadata.name);

        // send event
        let event = ReplicaSetEvent {
            event_type: EventType::Added,
            replicaset: rs.clone(),
        };
        let _ = self.replicaset_tx.send(event);
        Ok(rs.metadata.id)
    }

    /// Retrieves all replicasets.
    pub async fn get_replicasets(&self) -> Vec<ReplicaSet> {
        self.store.list_replicasets().await.unwrap_or_default()
    }

    /// Adds a new pod, assigns it a UUID, and emits a PodEvent.
    pub async fn add_pod(&self, spec: PodSpec, metadata: Metadata) -> Result<Uuid, StoreError> {
        // validate spec and name
        validate_container_list(&spec.containers)?;

        let pod = Pod {
            spec,
            metadata,
            status: PodStatus::default(),
        };

        // save object and metadata in store and cache
        self.store.put_pod(&pod.metadata.id, &pod).await?;
        self.cache.add_pod(&pod.metadata.name, pod.metadata.id);

        // send event
        let event = PodEvent {
            event_type: EventType::Added,
            pod: pod.clone(),
        };
        let _ = self.pod_tx.send(event);
        Ok(pod.metadata.id)
    }

    /// Deletes a pod by name and emits a deletion event.
    pub async fn delete_pod(&self, name: &str) -> Result<(), StoreError> {
        // get pod id
        let id = self
            .cache
            .get_pod_id(name)
            .ok_or_else(|| StoreError::NotFound("Pod not found".to_string()))?;
        // get object from store
        let pod = self
            .store
            .get_pod(id)
            .await?
            .ok_or_else(|| StoreError::NotFound("Pod not found".to_string()))?;

        // clean store and cache
        self.store.delete_pod(&id).await?;
        self.cache.delete_pod(name);

        // send delete event
        let event = PodEvent {
            event_type: EventType::Deleted,
            pod,
        };
        let _ = self.pod_tx.send(event);
        Ok(())
    }

    /// Assigns a pod to a node if unassigned and the node exists.
    pub async fn assign_pod(&self, name: &str, node_name: String) -> Result<(), StoreError> {
        // check node name exists
        (self.cache.node_name_exists(&node_name))
            .then_some(())
            .ok_or_else(|| {
                StoreError::InvalidReference(format!("No node exists with name={}", node_name))
            })?;

        // check pod name exists and its in unassigned set
        let Some(pod_id) = self.cache.get_pod_id(name) else {
            return Err(StoreError::NotFound(format!(
                "No pod exists with name={}",
                name
            )));
        };

        // check pod is unassigned
        let mut pod = self
            .store
            .get_pod(pod_id.clone())
            .await?
            .ok_or(StoreError::NotFound("Pod not found in store".to_string()))?;

        if !pod.spec.node_name.is_empty() {
            return Err(StoreError::Conflict(format!(
                "Pod ({}) is already assigned to a node",
                name
            )));
        }

        // assign ad store node
        pod.spec.node_name = node_name.clone();
        pod.metadata.generation += 1;
        self.store.put_pod(&pod.metadata.id, &pod).await?;

        // update cache, move from unassigned to node
        self.cache.assign_pod(name, &pod_id, &node_name);

        // send event
        let event = PodEvent {
            event_type: EventType::Modified,
            pod,
        };
        let _ = self.pod_tx.send(event);
        Ok(())
    }

    /// Updates the runtime status of a pod, including container statuses.
    pub async fn update_pod_status(
        &self,
        id: &Uuid,
        status: &mut PodStatus,
    ) -> Result<(), StoreError> {
        let mut pod = self
            .store
            .get_pod(*id)
            .await?
            .ok_or(StoreError::NotFound("Pod not found in store".to_string()))?;

        validate_container_statuses(&pod.spec, &mut status.container_status);
        pod.status = status.clone();
        pod.status.last_update = Some(Utc::now());
        self.store.put_pod(&id, &pod).await?;
        // send event
        let event = PodEvent {
            event_type: EventType::Modified,
            pod,
        };
        let _ = self.pod_tx.send(event);
        Ok(())
    }

    /// Retrieves all pods, or only those scheduled on a specific node.
    pub async fn get_pods(&self, query: Option<String>) -> Vec<Pod> {
        match query {
            Some(node_name) => {
                let Some(pod_ids_ref) = self.cache.get_pod_ids(&node_name) else {
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

    /// Adds a new node
    pub async fn add_node(&self, node: &Node) -> Result<(), StoreError> {
        // store in cache and store
        self.store.put_node(&node.name, node).await?;
        self.cache.add_node(&node.name, &node.addr);

        // send event
        let event = NodeEvent {
            event_type: EventType::Added,
            node: node.clone(),
        };
        let _ = self.node_tx.send(event);
        Ok(())
    }

    /// Retrieves all registered nodes.
    pub async fn get_nodes(&self) -> Vec<Node> {
        self.store.list_nodes().await.unwrap_or_default()
    }

    /// Fetches a single node by name.
    pub async fn get_node(&self, name: &str) -> Result<Option<Node>, StoreError> {
        self.store.get_node(name).await
    }

    /// Updates a node's heartbeat timestamp.
    pub async fn update_node_heartbeat(&self, node_name: &str) -> Result<(), StoreError> {
        let mut node = self
            .store
            .get_node(node_name)
            .await?
            .ok_or(StoreError::NotFound(format!(
                "Node {} not found in store",
                node_name
            )))?;
        node.last_heartbeat = Utc::now();
        self.store.put_node(node_name, &node).await
    }
}

/// Validates pod spec for duplicate container names.
fn validate_container_list(list: &Vec<ContainerSpec>) -> Result<(), StoreError> {
    let mut seen_names = HashSet::new();

    for container in list {
        if !seen_names.insert(&container.name) {
            return Err(StoreError::WrongFormat(format!(
                "Duplicate container name found: '{}'",
                container.name
            )));
        }
    }

    Ok(())
}

/// Cleans up container status list to only include valid container names from spec.
fn validate_container_statuses(spec: &PodSpec, container_statuses: &mut Vec<(String, String)>) {
    let valid_names: HashSet<_> = spec.containers.iter().map(|c| c.name.clone()).collect();

    // Filter out invalid entries
    container_statuses.retain(|(name, _)| valid_names.contains(name));

    let existing_names: HashSet<_> = container_statuses
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    // Insert default status for containers not included
    for container in &spec.containers {
        if !existing_names.contains(&container.name) {
            container_statuses.push((container.name.clone(), "EMPTY".to_string()));
        }
    }
}
