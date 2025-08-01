use std::time::Duration;

use bollard::secret::ContainerStateStatusEnum;
use reqwest::Client;
use shared::{api::PodStatusUpdate, models::PodStatus};
use tokio::time;

use crate::state::State;

pub async fn run(state: State) -> Result<(), String> {
    let mut interval = time::interval(Duration::from_secs(state.config.sync_loop.into()));
    tracing::info!(sync=%state.config.sync_loop, "Starting sync loop");
    loop {
        interval.tick().await;
        let client = Client::new();
        for p in state.list_pod_runtimes().iter() {
            let mut container_statuses: Vec<(String, String)> = Vec::new();
            // Over simplification obv
            let mut pod_status = PodStatus::Running;
            for c in p.containers.values() {
                let status = state.docker_mgr.get_container_status(&c.id).await;
                match status {
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
                node_name: state.node_name(),
            };
            let response = client
                .patch(format!(
                    "{}/pods/{}/status",
                    state.config.server_url, p.name
                ))
                .json(&update)
                .send()
                .await;

            match response {
                Ok(resp) => tracing::trace!(response=%resp.status(), "Status update sent"),
                Err(err) => tracing::warn!(error=%err, "Status update failed"),
            }
        }
    }
}
