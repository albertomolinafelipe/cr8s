//! # Pod Status Sync Loop
//!
//! This module defines a background task that periodically polls the state of all container
//! runtimes and reports their status back to the control plane.

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
    loop {
        interval.tick().await;
        run_iteration(&state).await?;
    }
}

pub async fn run_iteration(state: &State) -> Result<(), String> {
    let client = Client::new();
    for p in state.list_pod_runtimes().iter() {
        let mut container_statuses: Vec<(String, String)> = Vec::new();
        let mut pod_status = PodStatus::Running;

        for c in p.containers.values() {
            match state.docker_mgr.get_container_status(&c.id).await {
                Ok(s) => {
                    container_statuses.push((c.spec_name.clone(), s.to_string()));
                    if s != ContainerStateStatusEnum::RUNNING {
                        pod_status = PodStatus::Succeeded;
                    }
                }
                Err(e) => tracing::error!(error=%e, "Failed to get container status"),
            }
        }

        let update = PodStatusUpdate {
            status: pod_status,
            container_statuses,
            node_name: state.config.name.clone(),
        };

        if let Err(err) = client
            .patch(format!(
                "{}/pods/{}/status",
                state.config.server_url, p.name
            ))
            .json(&update)
            .send()
            .await
        {
            tracing::warn!(error=%err, "Status update failed");
        }
    }
    Ok(())
}
