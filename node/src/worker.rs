use tokio::sync::mpsc::Receiver;
use uuid::Uuid;
use crate::state::State;

pub async fn run(state: State, mut rx: Receiver<Uuid>) {
    tokio::spawn(async move {
        while let Some(pod_id) = rx.recv().await {
            let app_state = state.clone();
            tokio::spawn(async move {
                placeholder(app_state, pod_id).await;
            });
        }
    });
}

async fn placeholder(_state: State, id: Uuid) {
    tracing::info!("Working on id: {}", id);
}
