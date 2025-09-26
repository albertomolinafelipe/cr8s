use dashmap::{DashMap, DashSet};
use rand::Rng;
use rand::prelude::IndexedRandom;
use shared::models::{node::Node, pod::Pod};
use std::sync::Arc;
use uuid::Uuid;

pub type State = Arc<SchedulerState>;

/// In-memory scheduler state shared across tasks.
#[derive(Debug)]
pub struct SchedulerState {
    pub nodes: DashMap<String, Node>,
    pub pods: DashMap<Uuid, Pod>,
    pub pod_map: DashMap<String, DashSet<Uuid>>,

    pub pod_resources: DashMap<Uuid, SimResources>,
    pub node_resources: DashMap<String, SimResources>,

    pub pods_uri: String,
}

impl SchedulerState {
    pub fn new(apiserver: &str) -> State {
        Arc::new(Self {
            nodes: DashMap::new(),
            pods: DashMap::new(),
            pod_map: DashMap::new(),
            node_resources: DashMap::new(),
            pod_resources: DashMap::new(),
            pods_uri: format!("{}/pods", apiserver),
        })
    }

    pub fn add_pod(&self, pod: &Pod) {
        // add pod to map
        self.pods.insert(pod.metadata.id, pod.clone());
        self.pod_resources
            .insert(pod.metadata.id, SimResources::new_pod_res());
        // store pod in unassigned group
        self.pod_map
            .entry("".to_string())
            .or_insert_with(DashSet::new)
            .insert(pod.metadata.id);
        // send pod id to channel for scheduling
    }

    pub fn add_node(&self, node: &Node) {
        self.nodes.insert(node.name.clone(), node.clone());
        self.node_resources
            .insert(node.name.clone(), SimResources::new_node_res());
    }

    pub fn delete_pod(&self, id: &Uuid) {
        // remove pod
        if let Some((_, pod)) = self.pods.remove(id) {
            // remove resource
            if let Some((_, pod_res)) = self.pod_resources.remove(id) {
                if !pod.spec.node_name.is_empty() {
                    // add back if assigned
                    if let Some(mut node_res) = self.node_resources.get_mut(&pod.spec.node_name) {
                        node_res.add(&pod_res);
                    }
                }
            }
            // remove from map
            if let Some(set) = self.pod_map.get(&pod.spec.node_name) {
                set.remove(id);
            }
        } else {
            tracing::warn!(%id, "Failed to delete pod");
        }
    }

    pub fn assign_pod(&self, id: &Uuid, node: &str) {
        // figure out current assignment
        let current_node = self
            .pods
            .get(id)
            .map(|pod| pod.spec.node_name.clone())
            .unwrap_or_default();

        // get pod resources
        let pod_res = match self.pod_resources.get(id) {
            Some(r) => r.clone(),
            None => return,
        };

        // if currently assigned, free resources and remove from bucket
        if !current_node.is_empty() {
            if let Some(mut node_res) = self.node_resources.get_mut(&current_node) {
                node_res.add(&pod_res);
            }
        }
        if let Some(set) = self.pod_map.get(&current_node) {
            set.remove(id);
        }

        // add to new node bucket
        {
            let set = self
                .pod_map
                .entry(node.to_string())
                .or_insert_with(DashSet::new);
            set.insert(*id);
        }

        // subtract pod resources from new node
        if let Some(mut node_res) = self.node_resources.get_mut(node) {
            node_res.sub(&pod_res);
        }

        // update pod assignment in pods map
        drop(current_node);
        if let Some(mut pod) = self.pods.get_mut(id) {
            pod.spec.node_name = node.to_string();
        }
    }
}

// -------------------------

#[derive(Debug, Clone)]
pub struct SimResources {
    pub cpu: u64,
    pub mem: u64,
}

impl SimResources {
    pub fn new_node_res() -> Self {
        let mut rng = rand::rng();

        // 1 core, 2 cores, 4 cores
        let cpu_options = [1000, 2000, 4000];
        let cpu = *cpu_options.choose(&mut rng).unwrap();

        // 2 GiB, 4 GiB, 8 GiB
        let mem_options = [
            2 * 1024 * 1024 * 1024,
            4 * 1024 * 1024 * 1024,
            8 * 1024 * 1024 * 1024,
        ];
        let mem = *mem_options.choose(&mut rng).unwrap();

        Self { cpu, mem }
    }

    pub fn new_pod_res() -> Self {
        let mut rng = rand::rng();
        Self {
            cpu: rng.random_range(100..=1000),
            mem: rng.random_range(64..=512) * 1024 * 1024,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.cpu += other.cpu;
        self.mem += other.mem;
    }

    /// Subtract resources (used when assigning pods)
    pub fn sub(&mut self, other: &Self) {
        self.cpu = self.cpu.saturating_sub(other.cpu);
        self.mem = self.mem.saturating_sub(other.mem);
    }
}
