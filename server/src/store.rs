use actix_web::HttpResponse;
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use etcd_client::GetOptions;
use futures::future::join_all;
use std::{collections::HashSet, fmt};
use tokio::sync::broadcast;
use uuid::Uuid;

use shared::{
    api::{EventType, NodeEvent, PodEvent},
    models::{Metadata, Node, PodObject, PodSpec, PodStatus, UserMetadata},
};

pub enum StoreError {
    WrongFormat(String),
    Conflict(String),
    UnexpectedError(String),
    NotFound(String),
    InvalidReference(String),
}

impl StoreError {
    pub fn to_http_response(&self) -> HttpResponse {
        match self {
            StoreError::WrongFormat(msg) => HttpResponse::BadRequest().body(msg.clone()),
            StoreError::Conflict(msg) => HttpResponse::Conflict().body(msg.clone()),
            StoreError::NotFound(msg) => HttpResponse::NotFound().body(msg.clone()),
            StoreError::InvalidReference(msg) => {
                HttpResponse::UnprocessableEntity().body(msg.clone())
            }
            StoreError::UnexpectedError(_) => {
                HttpResponse::InternalServerError().body("Unexpected error")
            }
        }
    }
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::WrongFormat(msg) => write!(f, "Wrong format: {}", msg),
            StoreError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            StoreError::UnexpectedError(msg) => write!(f, "Unexpected error: {}", msg),
            StoreError::NotFound(msg) => write!(f, "Not found error: {}", msg),
            StoreError::InvalidReference(msg) => write!(f, "Invalid reference error: {}", msg),
        }
    }
}

pub struct R8s {
    etcd: etcd_client::Client,
    pub pod_tx: broadcast::Sender<PodEvent>,
    pub node_tx: broadcast::Sender<NodeEvent>,

    pub node_names: DashSet<String>,
    node_addrs: DashSet<String>,
    /// Assigned pods per node
    pub pod_map: DashMap<String, DashSet<Uuid>>,
    pub pod_name_idx: DashMap<String, Uuid>,
    // ETCD storage
    // /pods/{pod_id}
    // /nodes/{node_name}
}

impl R8s {
    pub async fn new() -> Self {
        let (pod_tx, _) = broadcast::channel(10);
        let (node_tx, _) = broadcast::channel(10);
        let etcd_addr =
            std::env::var("ETCD_ADDR").unwrap_or_else(|_| "http://etcd:2379".to_string());
        tracing::info!(addr=%etcd_addr, "Connecting to etcd");
        let etcd = etcd_client::Client::connect([&etcd_addr], None)
            .await
            .expect("Failed to connect to etcd");
        Self {
            etcd,
            pod_tx,
            node_tx,
            node_names: DashSet::new(),
            node_addrs: DashSet::new(),
            pod_map: DashMap::new(),
            pod_name_idx: DashMap::new(),
        }
    }

    pub async fn add_pod(&self, spec: PodSpec, metadata: UserMetadata) -> Result<Uuid, StoreError> {
        // validate spec and name
        validate_pod(&spec)?;
        (!self.pod_name_idx.contains_key(&metadata.name))
            .then_some(())
            .ok_or_else(|| StoreError::Conflict("Duplicate pod name".to_string()))?;

        // Since its low level object manifest is not stored
        let pod = PodObject {
            id: Uuid::new_v4(),
            node_name: "".to_string(),
            pod_status: PodStatus::Pending,
            metadata: Metadata::new(metadata),
            spec,
        };
        let key = format!("/pods/{}", pod.id);
        let value =
            serde_json::to_string(&pod).map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        // Into store and name index
        let mut etcd = self.etcd.clone();
        etcd.put(key, value, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        self.pod_name_idx
            .insert(pod.metadata.user.name.clone(), pod.id);
        self.pod_map
            .entry("".to_string())
            .or_insert_with(DashSet::new)
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

        let unassigned_entry = self
            .pod_map
            .entry("".to_string())
            .or_insert_with(DashSet::new);
        if !unassigned_entry.contains(&*pod_id) {
            return Err(StoreError::Conflict(format!(
                "Pod ({}) is not in the unassigned set",
                name
            )));
        }

        // Check pod is unassigned
        let mut pod = self.fetch_pod(*pod_id).await?;
        if !pod.node_name.is_empty() {
            return Err(StoreError::Conflict(format!(
                "Pod ({}) is already assigned to a node",
                name
            )));
        }

        // Assign and insert pod
        pod.node_name = node_name.clone();
        let key = format!("/pods/{}", pod.id);
        let value =
            serde_json::to_string(&pod).map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        let mut etcd = self.etcd.clone();
        etcd.put(key, value, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

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

    pub async fn get_pods(&self, query: Option<String>) -> Vec<PodObject> {
        match query {
            Some(node_name) => {
                let Some(pod_ids_ref) = self.pod_map.get(&node_name) else {
                    return vec![];
                };
                join_all(pod_ids_ref.iter().map(|id| self.fetch_pod(*id)))
                    .await
                    .into_iter()
                    .inspect(|res| {
                        if let Err(e) = res {
                            tracing::error!(error=%e, "Error fetching pod");
                        }
                    })
                    .filter_map(Result::ok)
                    .collect()
            }
            None => {
                let mut etcd = self.etcd.clone();
                let resp = match etcd
                    .get("/pods/", Some(GetOptions::new().with_prefix()))
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        tracing::error!(error=%e, "Could not fetch pods");
                        return vec![];
                    }
                };
                resp.kvs()
                    .iter()
                    .filter_map(|kv| {
                        kv.value_str()
                            .ok()
                            .and_then(|val| serde_json::from_str::<PodObject>(val).ok())
                    })
                    .collect()
            }
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
        let key = format!("/nodes/{}", node.name);
        let value =
            serde_json::to_string(node).map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        let mut etcd = self.etcd.clone();
        etcd.put(key, value, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        // Into index
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
        let mut etcd = self.etcd.clone();

        let resp = match etcd
            .get("/nodes/", Some(GetOptions::new().with_prefix()))
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(error=%e, "Could not fetch nodes");
                return vec![];
            }
        };

        resp.kvs()
            .iter()
            .filter_map(|kv| {
                kv.value_str()
                    .ok()
                    .and_then(|val| serde_json::from_str::<Node>(val).ok())
            })
            .collect()
    }

    pub async fn update_node_heartbeat(&self, node_name: &str) -> Result<(), StoreError> {
        let key = format!("/nodes/{}", node_name);

        let mut etcd = self.etcd.clone();
        let resp = etcd
            .get(key.clone(), None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        let Some(kv) = resp.kvs().first() else {
            return Err(StoreError::NotFound(format!(
                "Node {} not found",
                node_name
            )));
        };

        let val_str = kv
            .value_str()
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        let mut node: Node = serde_json::from_str(val_str)
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        node.last_heartbeat = Utc::now();

        let new_val =
            serde_json::to_string(&node).map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        etcd.put(key, new_val, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        Ok(())
    }

    async fn fetch_pod(&self, id: Uuid) -> Result<PodObject, StoreError> {
        let mut etcd = self.etcd.clone();
        let key = format!("/pods/{}", id);

        let resp = etcd
            .get(key, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        let kv = resp
            .kvs()
            .first()
            .ok_or_else(|| StoreError::UnexpectedError(format!("No pod with id={}", id)))?
            .value_str()
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

        let pod: PodObject = serde_json::from_str(kv)
            .map_err(|e| StoreError::UnexpectedError(format!("Deserialization error: {}", e)))?;

        Ok(pod)
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
