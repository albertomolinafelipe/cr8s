//! SYNC LOOP TESTS
//!
//! - test_sync_no_pods, no pods to report
//! - test_sync_loop, should
//!     call docker api
//!     update node state
//!     send call to server

mod common;

use bollard::secret::ContainerStateStatusEnum;
use common::test_docker::TestDocker;
use common::utils::start_sync;
use r8sagt::state::new_state_with;
use r8sagt::{core::worker, models::Config};
use shared::models::{ContainerSpec, PodObject, PodSpec};

use crate::common::utils::start_mock_server;

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
