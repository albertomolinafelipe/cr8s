//! # Pod Status Sync Loop
//!
//! This module defines a background task that periodically polls the state of all container
//! runtimes and reports their status back to the control plane.

use std::collections::HashMap;
use std::time::Duration;

use bollard::secret::ContainerStateStatusEnum;
use reqwest::Client;
use shared::{api::PodStatusUpdate, models::PodStatus};
use tokio::time;

use crate::state::State;

/// Starts the periodic pod status sync loop.
///
/// This continuously polls container states via the Docker manager and sends status updates
/// to the control plane via PATCH requests.
///
/// Loop interval is defined by the `SYNC_LOOP_INTERVAL` environment variable (in seconds).

pub async fn run(state: State) -> Result<(), String> {
    tracing::info!(sync=%state.config.sync_loop, "Starting sync loop");
    let mut interval = time::interval(Duration::from_secs(state.config.sync_loop.into()));
    let client = Client::new();

    loop {
        interval.tick().await;

        for p in state.list_pod_runtimes().iter() {
            let mut container_statuses_map: HashMap<String, ContainerStateStatusEnum> =
                HashMap::new();
            let mut container_statuses_for_update: Vec<(String, String)> = Vec::new();
            let mut pod_status = PodStatus::Running;

            for c in p.containers.values() {
                match state.docker_mgr.get_container_status(&c.id).await {
                    Ok(s) => {
                        container_statuses_map.insert(c.spec_name.clone(), s.clone());
                        container_statuses_for_update.push((c.spec_name.clone(), s.to_string()));
                        if s != ContainerStateStatusEnum::RUNNING {
                            pod_status = PodStatus::Succeeded;
                        }
                    }
                    Err(e) => tracing::error!(error=%e, "Failed to get container status"),
                }
            }

            // Update the in-memory runtime state for this pod
            if let Err(err) = state.update_pod_runtime_status(&p.id, container_statuses_map) {
                tracing::warn!(error=%err, "Failed to update pod runtime status in-memory");
            }

            // Build and send status update to control plane
            let update = PodStatusUpdate {
                status: pod_status,
                container_statuses: container_statuses_for_update,
                node_name: state.config.name.clone(),
            };

            let response = client
                .patch(format!("{}/pods/{}", state.config.server_url, p.name))
                .json(&update)
                .send()
                .await;

            if let Err(err) = response {
                tracing::warn!(error=%err, "Status update failed");
            }
        }
    }
}
