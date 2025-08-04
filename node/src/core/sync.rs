//! # Pod Status Sync Loop
//!
//! This module defines a background task that periodically polls the state of all container
//! runtimes and reports their status back to the control plane.

use std::{collections::HashMap, time::Duration};

use bollard::secret::ContainerStateStatusEnum;
use reqwest::Client;
use shared::{
    api::{PodField, PodPatch, PodStatusUpdate},
    models::PodStatus,
};
use tokio::time;

use crate::state::State;

/// Starts the periodic pod status sync loop.
///
/// This continuously polls container states via the Docker manager and sends status updates
/// to the control plane via PATCH requests.
pub async fn run(state: State) -> Result<(), String> {
    tracing::info!(sync=%state.config.sync_loop, "Starting sync loop");
    let mut interval = time::interval(Duration::from_secs(state.config.sync_loop.into()));
    loop {
        interval.tick().await;
        run_iteration(&state).await?;
    }
}

pub async fn run_iteration(state: &State) -> Result<(), String> {
    let client = Client::new();
    for p in state.list_pod_runtimes().iter() {
        let mut container_statuses_map: HashMap<String, ContainerStateStatusEnum> = HashMap::new();

        for c in p.containers.values() {
            match state.docker_mgr.get_container_status(&c.id).await {
                Ok(s) => {
                    container_statuses_map.insert(c.spec_name.clone(), s.clone());
                }
                Err(e) => tracing::error!(error=%e, "Failed to get container status"),
            };
        }

        // Update the in-memory runtime state for this pod
        let pod_status =
            match state.update_pod_runtime_status(&p.id, container_statuses_map.clone()) {
                Ok(status) => status,
                Err(err) => {
                    tracing::warn!(error=%err, "Failed to update pod runtime status in-memory");
                    PodStatus::Unknown
                }
            };

        // Build and send status update to control plane
        let Ok(update) = serde_json::to_value(PodStatusUpdate {
            status: pod_status,
            container_statuses: container_statuses_map
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
            node_name: state.config.name.clone(),
        }) else {
            continue;
        };
        let payload = PodPatch {
            pod_field: PodField::Status,
            value: update,
        };

        if let Err(err) = client
            .patch(format!("{}/pods/{}", state.config.server_url, p.name))
            .json(&payload)
            .send()
            .await
        {
            tracing::warn!(error=%err, "Status update failed");
        };
    }
    Ok(())
}
