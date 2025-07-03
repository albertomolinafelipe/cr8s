use reqwest::Client;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;
use crate::state::State;

pub async fn run(state: State, mut rx: Receiver<Uuid>) {
    tokio::spawn(async move {
        while let Some(pod_id) = rx.recv().await {
            let app_state = state.clone();
            tokio::spawn(async move {
                reconciliate(app_state, pod_id).await;
            });
        }
    });
}

async fn reconciliate(state: State, id: Uuid) {
    
    let Some(pod) = state.get_pod(&id) else {
        tracing::warn!("Pod {}, not found in pod manager", id);
        return;
    };
    // Check runtime state
    if let Some(_) = state.get_pod_runtime(&pod.id) {
        tracing::error!("Pod already stored in runtime state, not implemented");
        return;
    }

    let runtime = state.docker_mgr.start_pod(pod).await;
    state.add_pod_runtime(runtime).ok();

    // Take action to fix it
    let client = Client::new();
    let response = client
        .post(format!("{}/nodes/nodeName", state.config.server_url))
        .send()
        .await
        .unwrap();
    println!("Received {} from control-plane status endpoint", response.status());
}
