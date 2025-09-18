use reqwest::Client;
use serde_json::Value;
use shared::{
    api::{PodField, PodPatch},
    models::pod::Pod,
};
use uuid::Uuid;

use super::state::State;

/// Returns available nodes
/// Just checks cpu and mem for now
pub fn filter_nodes(state: &State, pod: &Pod) -> Vec<String> {
    let mut candidates = Vec::new();

    // get the pod's simulated resource requirements
    let Some(pod_res) = state.pod_resources.get(&pod.metadata.id) else {
        tracing::warn!(pod_name=%pod.metadata.name, "Pod has no simulated resources");
        return candidates;
    };

    // iterate through nodes and check capacity
    for entry in state.nodes.iter() {
        let node_name = entry.key();
        if let Some(node_res) = state.node_resources.get(node_name) {
            if node_res.cpu >= pod_res.cpu && node_res.mem >= pod_res.mem {
                candidates.push(node_name.clone());
            }
        }
    }

    candidates
}

/// Returns the name of the best node for this pod,
/// defined as the node with the most free CPU+mem after assignment.
pub fn best_node(state: &State, pod: &Pod) -> Option<String> {
    let pod_res = match state.pod_resources.get(&pod.metadata.id) {
        Some(r) => r.clone(),
        None => {
            tracing::warn!(pod_name=%pod.metadata.name, "Pod has no simulated resources");
            return None;
        }
    };

    // only consider feasible nodes
    let candidates = filter_nodes(state, pod);
    if candidates.is_empty() {
        return None;
    }

    // best = (node_name, pod_count, free_cpu, free_mem)
    let mut best: Option<(String, usize, u64, u64)> = None;

    for node_name in candidates {
        if let Some(node_res) = state.node_resources.get(&node_name) {
            let free_cpu = node_res.cpu - pod_res.cpu;
            let free_mem = node_res.mem - pod_res.mem;

            // number of active pods on this node
            let pod_count = state
                .pod_map
                .get(&node_name)
                .map(|set| set.len())
                .unwrap_or(0);

            match &best {
                None => {
                    best = Some((node_name, pod_count, free_cpu, free_mem));
                }
                Some((_, best_count, best_cpu, best_mem)) => {
                    if pod_count < *best_count
                        || (pod_count == *best_count
                            && (free_cpu > *best_cpu
                                || (free_cpu == *best_cpu && free_mem > *best_mem)))
                    {
                        best = Some((node_name, pod_count, free_cpu, free_mem));
                    }
                }
            }
        }
    }

    best.map(|(node, _, _, _)| node)
}

/// Assigns a pod to the best available node by patching the API server.
pub async fn schedule(state: State, id: Uuid) {
    let pod = match state.pods.get(&id) {
        Some(p) => p,
        None => {
            tracing::warn!(%id, "Pod not found in state");
            return;
        }
    };

    // Pick the best node among candidates
    let node = match best_node(&state, &pod) {
        Some(n) => n,
        None => {
            tracing::warn!(pod_name=%pod.metadata.name, "No suitable node found after scoring");
            return;
        }
    };

    // make patch call to api server
    let patch = PodPatch {
        pod_field: PodField::NodeName,
        value: Value::String(node.clone()),
    };

    let client = Client::new();
    let base_url = state
        .api_server
        .as_deref()
        .unwrap_or("http://localhost:7620");
    let url = format!("{}/pods/{}", base_url, pod.metadata.name);

    match client.patch(&url).json(&patch).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                pod=%pod.metadata.name,
                %node,
                "Scheduled"
            );
            drop(pod);
            // Move pod to its assigned node group
            state.assign_pod(&id, &node);
        }
        Ok(resp) => {
            tracing::error!(
                status = %resp.status(),
                "Failed to patch pod: non-success response"
            );
        }
        Err(err) => {
            tracing::error!("Failed to patch pod: {}", err);
        }
    }
}
