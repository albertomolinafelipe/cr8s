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

#[cfg(test)]
mod tests {

    //! - test_sync_no_pods, no pods to report
    //! - test_sync_loop, should
    //!     call docker api
    //!     update node state
    //!     send call to server

    use std::sync::Arc;

    use crate::{core::worker, docker::test::TestDocker, models::Config, state::new_state_with};

    use super::*;
    use bollard::secret::ContainerStateStatusEnum;
    use shared::models::{ContainerSpec, PodObject, PodSpec};
    use tokio::sync::Notify;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path_regex},
    };

    async fn start_sync(state: State) -> (Arc<Notify>, tokio::task::JoinHandle<()>) {
        let notify = Arc::new(Notify::new());
        let notify_clone = notify.clone();

        run_iteration(&state).await.unwrap();
        let handle = tokio::spawn(async move {
            loop {
                notify_clone.notified().await;
                run_iteration(&state).await.unwrap();
            }
        });

        (notify, handle)
    }

    async fn start_mock_server() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path_regex(r"^/pods/[^/]+/status$"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        server
    }
    #[tokio::test]
    async fn test_sync_no_pods() {
        let docker = Box::new(TestDocker::new());
        let mock_server = start_mock_server().await;
        let config = Config {
            server_url: mock_server.uri(),
            ..Default::default()
        };
        let state = new_state_with(Some(config), Some(docker.clone()));

        let mock_server = start_mock_server().await;
        let (_, handle) = start_sync(state.clone()).await;
        handle.abort();

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 0);
        assert_eq!(docker.get_container_status_calls.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_loop() {
        let mut docker = Box::new(TestDocker::new());
        docker.start_pod_default_status = Some(ContainerStateStatusEnum::EMPTY);
        let mock_server = start_mock_server().await;
        let config = Config {
            server_url: mock_server.uri(),
            ..Default::default()
        };
        let state = new_state_with(Some(config), Some(docker.clone()));
        let pod = PodObject {
            spec: PodSpec {
                containers: vec![ContainerSpec::default(), ContainerSpec::default()],
            },
            ..Default::default()
        };
        // create and add pod to state
        state.put_pod(&pod);
        worker::reconciliate(state.clone(), pod.id).await;
        docker.set_all_container_statuses(ContainerStateStatusEnum::RUNNING);
        assert!(state.list_pod_runtimes().len() != 0);

        // start server and sync loop
        let (_, handle) = start_sync(state.clone()).await;
        handle.abort();

        // should have called for every container in the pod
        assert_eq!(docker.get_container_status_calls.lock().await.len(), 2);
        // one call in iteration
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        // update node state, should read running
        assert!(
            state
                .get_pod_runtime(&pod.id)
                .unwrap()
                .containers
                .values()
                .all(|c| c.status == ContainerStateStatusEnum::RUNNING)
        );
    }
}
