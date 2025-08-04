//! SYNC LOOP TESTS
//!
//! - test_sync_no_pods, no pods to report
//! - test_sync_loop, should
//!     call docker api
//!     update node state
//!     send call to server

mod common;
use common::test_docker::TestDocker;
use common::utils::start_sync;
use r8sagt::models::Config;
use r8sagt::state::new_state_with;

use crate::common::utils::start_mock_server;

#[tokio::test]
async fn test_sync_no_pods() {
    let docker = Box::new(TestDocker::new());
    let state = new_state_with(Some(Config::default()), Some(docker.clone()));

    let mock_server = start_mock_server().await;
    let (_, handle) = start_sync(state.clone());
    handle.abort();

    let requests = mock_server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 0);
    assert_eq!(docker.get_container_status_calls.lock().await.len(), 0);
}

//  #[tokio::test]
//  async fn test_sync_loop() {
//      let docker = Box::new(TestDocker::new());
//      let state = new_state_with(Some(Config::default()), Some(docker.clone()));
//      let pod = PodObject {
//          spec: PodSpec {
//              containers: vec![ContainerSpec::default(), ContainerSpec::default()],
//          },
//          ..Default::default()
//      };
//      // create and add pod to state
//      let runtime = docker.start_pod(pod).await.unwrap();
//      state.add_pod_runtime(runtime).unwrap();

//      // start server and sync loop
//      let mock_server = start_mock_server().await;
//      let (_, handle) = start_sync(state.clone());
//      handle.abort();

//      // should have called for every container in the pod
//      assert_eq!(docker.get_container_status_calls.lock().await.len(), 2);

//      state.get_pod_runtime(&runtime.id).

//      let requests = mock_server.received_requests().await.unwrap();
//      assert_eq!(requests.len(), 1);
//  }
