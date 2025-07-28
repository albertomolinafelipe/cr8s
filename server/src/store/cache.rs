use dashmap::{DashMap, DashSet};
use uuid::Uuid;

const UNASSIGNED_NODE: &str = "";

struct PodInfo {
    node: String,
    id: Uuid,
}

pub struct CacheManager {
    /// List of node names
    node_names: DashSet<String>,
    /// List of node addresses
    node_addrs: DashSet<String>,

    /// Set of pods assigned to each node
    pod_map: DashMap<String, DashSet<Uuid>>,
    /// Map pod name to assignment and id
    pod_name_idx: DashMap<String, PodInfo>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            node_names: DashSet::new(),
            node_addrs: DashSet::new(),
            pod_map: DashMap::new(),
            pod_name_idx: DashMap::new(),
        }
    }

    pub fn node_name_exists(&self, name: &str) -> bool {
        self.node_names.contains(name)
    }

    pub fn node_addr_exists(&self, addr: &str) -> bool {
        self.node_addrs.contains(addr)
    }

    pub fn add_node(&self, name: &str, addr: &str) {
        self.node_addrs.insert(addr.to_string());
        self.node_names.insert(name.to_string());
    }

    pub fn pod_name_exists(&self, name: &str) -> bool {
        self.pod_name_idx.contains_key(name)
    }

    pub fn get_pod_id(&self, name: &str) -> Option<Uuid> {
        self.pod_name_idx.get(name).map(|s| s.id)
    }

    pub fn get_pod_ids(&self, node_name: &str) -> Option<DashSet<Uuid>> {
        self.pod_map.get(node_name).map(|set_ref| set_ref.clone())
    }

    pub fn add_pod(&self, name: &str, id: Uuid) {
        self.pod_name_idx.insert(
            name.to_string(),
            PodInfo {
                node: "".to_string(),
                id: id,
            },
        );
        self.pod_map.entry("".to_string()).or_default().insert(id);
    }

    pub fn assign_pod(&self, pod_name: &str, pod_id: &Uuid, node_name: &str) {
        if let Some(mut pod_info) = self.pod_name_idx.get_mut(pod_name) {
            pod_info.node = node_name.to_string();
        }
        self.pod_map
            .entry(node_name.to_string())
            .or_insert_with(DashSet::new)
            .insert(*pod_id);
    }
}
