//! Drift-controller
//! Watch and delete broken pods

use std::sync::Arc;

use dashmap::DashMap;
use shared::{
    api::{EventType, PodEvent},
    models::pod::{Pod, PodPhase},
    utils::watch_stream,
};
use uuid::Uuid;

type State = Arc<GCState>;

pub async fn run() {
    watch_pods(Arc::new(GCState::new())).await.expect(".")
}

/// In-memory scheduler state shared across tasks.
#[derive(Debug)]
struct GCState {
    _pods: DashMap<Uuid, Pod>,
}

impl GCState {
    fn new() -> Self {
        Self {
            _pods: DashMap::new(),
        }
    }
}

async fn watch_pods(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/pods?watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        let gc_state = state.clone();
        tokio::spawn(async move {
            handle_pod_event(gc_state.clone(), event).await;
        });
    })
    .await;
    Ok(())
}

/// Track pod and trigger scheduling.
async fn handle_pod_event(_state: State, event: PodEvent) {
    match event.event_type {
        EventType::Modified => match event.pod.status.phase {
            PodPhase::Failed | PodPhase::Succeeded => {
                let pod = event.pod.metadata.name;
                let url = format!("http://localhost:7620/pods/{}", pod);
                tracing::info!(%pod, "Deleting");

                if let Err(err) = reqwest::Client::new().delete(&url).send().await {
                    tracing::error!("Failed to delete pod {}: {}", pod, err);
                    return;
                }
            }
            _ => {}
        },
        _ => {}
    }
}
