use std::sync::Arc;

use r8sagt::{core::sync, state::State};
use tokio::sync::Notify;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path_regex},
};

/// Starts sync loop and returns notify for coordination
pub async fn start_sync(state: State) -> (Arc<Notify>, tokio::task::JoinHandle<()>) {
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    sync::run_iteration(&state).await.unwrap();
    let handle = tokio::spawn(async move {
        loop {
            notify_clone.notified().await;
            sync::run_iteration(&state).await.unwrap();
        }
    });

    (notify, handle)
}

pub async fn start_mock_server() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path_regex(r"^/pods/[^/]+/status$"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    server
}
