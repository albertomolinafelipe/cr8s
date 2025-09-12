use dashmap::DashSet;
use rand::seq::IteratorRandom;
use reqwest::Client;
use serde_json::Value;
use shared::api::{PodField, PodPatch};
use uuid::Uuid;

use super::state::State;

/// Assigns a pod to a random available node by patching the API server.
pub async fn schedule(state: State, id: Uuid) {
    let pod = match state.pods.get(&id) {
        Some(p) => p,
        None => {
            tracing::warn!(%id, "Pod not found in state");
            return;
        }
    };

    // Pick a random node
    let node = match state.nodes.iter().choose(&mut rand::rng()) {
        Some(entry) => entry.key().clone(),
        None => {
            tracing::warn!("No available nodes to schedule onto");
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
                pod_name=%pod.metadata.name,
                node_name=%node,
                "Scheduled pod"
            );
            // Move pod to its assigned node group
            state
                .pod_map
                .entry("".to_string())
                .or_insert_with(DashSet::new)
                .remove(&id);
            state
                .pod_map
                .entry(node)
                .or_insert_with(DashSet::new)
                .insert(id);
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
