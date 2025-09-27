use std::collections::{HashMap, HashSet};

use dashmap::{DashMap, DashSet};
use uuid::Uuid;

/// Stores metadata about a pod, including its ID and assigned node.
#[derive(Clone)]
pub struct PodInfo {
    pub node: String,
    pub id: Uuid,
}

/// An in-memory concurrent cache for tracking node and pod assignments.
pub struct CacheManager {
    /// Set of known node names
    node_names: DashSet<String>,
    /// Set of known node addresses
    node_addrs: DashSet<String>,

    /// Maps node name to a set of pod IDs currently scheduled on that node.
    pod_map: DashMap<String, DashSet<Uuid>>,
    /// Maps pod name to its associated info (node assignment and UUID).
    pod_name_idx: DashMap<String, PodInfo>,

    /// Set of know rs names
    replicaset_names: DashSet<String>,
    /// Labels lookups
    pod_label_idx: DashMap<String, DashMap<String, DashSet<Uuid>>>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            node_names: DashSet::new(),
            node_addrs: DashSet::new(),
            pod_map: DashMap::new(),
            pod_name_idx: DashMap::new(),
            replicaset_names: DashSet::new(),
            pod_label_idx: DashMap::new(),
        }
    }

    // --- Node ops ---
    //
    // - Check name and addr duplicates
    // - Add to cache

    pub fn node_name_exists(&self, name: &str) -> bool {
        self.node_names.contains(name)
    }

    pub fn node_addr_exists(&self, addr: &str) -> bool {
        self.node_addrs.contains(addr)
    }

    /// Adds a node name and address to the cache.
    pub fn add_node(&self, name: &str, addr: &str) {
        self.node_addrs.insert(addr.to_string());
        self.node_names.insert(name.to_string());
    }

    // --- RS ops ---
    //
    // - Check name duplicates
    // - Add to cache

    pub fn replicaset_name_exists(&self, name: &str) -> bool {
        self.replicaset_names.contains(name)
    }

    pub fn add_replicaset(&self, name: &str) {
        self.replicaset_names.insert(name.to_string());
    }

    // --- Pod ops ---
    //
    // - Check name duplicates
    // - Get pod info by name
    // - Add pod
    // - Delete pod
    // - Assign pod

    pub fn pod_name_exists(&self, name: &str) -> bool {
        self.pod_name_idx.contains_key(name)
    }

    /// Retrieves the UUID of the pod with the given name.
    pub fn get_pod_id(&self, name: &str) -> Option<Uuid> {
        self.pod_name_idx.get(name).map(|s| s.id)
    }

    /// Returns full pod info
    pub fn get_pod_info(&self, name: &str) -> Option<PodInfo> {
        self.pod_name_idx.get(name).map(|entry| entry.clone())
    }

    /// Returns all pod IDs assigned to the specified node.
    pub fn get_pod_ids(&self, node_name: &str) -> Option<DashSet<Uuid>> {
        self.pod_map.get(node_name).map(|set_ref| set_ref.clone())
    }

    /// Inserts a pod with no node assignment initially.
    pub fn add_pod(&self, name: &str, id: &Uuid) {
        self.pod_name_idx.insert(
            name.to_string(),
            PodInfo {
                node: "".to_string(),
                id: id.clone(),
            },
        );
        self.pod_map
            .entry("".to_string())
            .or_default()
            .insert(id.clone());
    }

    /// Deletes a pod from both the pod map and node assignment.
    pub fn delete_pod(&self, name: &str) {
        if let Some((_, pod_info)) = self.pod_name_idx.remove(name) {
            if let Some(set) = self.pod_map.get(&pod_info.node) {
                set.remove(&pod_info.id);
            }
        }
    }

    /// Assigns a pod to a node and updates all relevant mappings.
    pub fn assign_pod(&self, pod_name: &str, pod_id: &Uuid, node_name: &str) {
        // Update the pod_info mapping
        if let Some(mut pod_info) = self.pod_name_idx.get_mut(pod_name) {
            // Remove from previous assignment if needed
            if let Some(prev_set) = self.pod_map.get(&pod_info.node) {
                prev_set.remove(pod_id);
            }

            pod_info.node = node_name.to_string();
        }

        // Add to new node's set
        self.pod_map
            .entry(node_name.to_string())
            .or_insert_with(DashSet::new)
            .insert(*pod_id);
    }

    pub fn add_pod_labels(&self, pod_id: &Uuid, labels: &HashMap<String, String>) {
        for (k, v) in labels {
            let inner = self
                .pod_label_idx
                .entry(k.clone())
                .or_insert_with(DashMap::new);
            let set = inner.entry(v.clone()).or_insert_with(DashSet::new);
            set.insert(pod_id.clone());
        }
    }

    pub fn remove_pod_labels(&self, pod_id: &Uuid, labels: &HashMap<String, String>) {
        for (k, v) in labels {
            if let Some(inner) = self.pod_label_idx.get(k) {
                if let Some(set) = inner.get(v) {
                    set.remove(pod_id);
                }
            }
        }
    }
    pub fn query_pods_by_labels(&self, labels: &HashMap<String, String>) -> Vec<Uuid> {
        let mut sets: Vec<Vec<Uuid>> = Vec::new();

        for (k, v) in labels {
            if let Some(inner) = self.pod_label_idx.get(k) {
                if let Some(set) = inner.get(v) {
                    sets.push(set.iter().map(|id| *id).collect());
                } else {
                    // no pods match
                    return Vec::new();
                }
            } else {
                // key not found
                return Vec::new();
            }
        }

        if sets.is_empty() {
            return Vec::new();
        }

        // intersect all sets
        let mut intersection: HashSet<Uuid> = sets[0].iter().copied().collect();
        for s in sets.iter().skip(1) {
            intersection = intersection
                .intersection(&s.iter().copied().collect())
                .copied()
                .collect();
        }

        intersection.into_iter().collect()
    }

    pub fn query_pods(
        &self,
        node_name: &Option<String>,
        labels: &HashMap<String, String>,
    ) -> Vec<Uuid> {
        let mut pod_sets: Vec<std::collections::HashSet<Uuid>> = Vec::new();

        // by node name
        if let Some(node) = node_name {
            if let Some(node_set) = self.pod_map.get(node) {
                pod_sets.push(node_set.clone().into_iter().collect());
            } else {
                return Vec::new();
            }
        }

        // by labels
        if !labels.is_empty() {
            let label_pods = self.query_pods_by_labels(labels);
            if label_pods.is_empty() {
                return Vec::new();
            }
            pod_sets.push(label_pods.into_iter().collect());
        }

        //return all pod IDs
        if pod_sets.is_empty() {
            let all_pods: std::collections::HashSet<Uuid> =
                self.pod_name_idx.iter().map(|e| e.id).collect();
            return all_pods.into_iter().collect();
        }

        // intersect
        let mut intersection = pod_sets[0].clone();
        for s in pod_sets.iter().skip(1) {
            intersection = intersection.intersection(s).copied().collect();
        }

        intersection.into_iter().collect()
    }
}
