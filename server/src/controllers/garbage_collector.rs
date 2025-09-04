//! Drift-controller
//! Watch and delete broken pods

use std::sync::Arc;

use dashmap::DashMap;
use shared::{
    api::{EventType, PodEvent},
    models::PodObject,
    utils::watch_stream,
};
use uuid::Uuid;

type State = Arc<GCState>;

pub async fn run() {
    tracing::info!("Initializing");
    watch_pods(Arc::new(GCState::new())).await.expect(".")
}

/// In-memory scheduler state shared across tasks.
#[derive(Debug)]
struct GCState {
    pods: DashMap<Uuid, PodObject>,
}

impl GCState {
    fn new() -> Self {
        Self {
            pods: DashMap::new(),
        }
    }
}

async fn watch_pods(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/pods?watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        handle_pod_event(state.clone(), event);
    })
    .await;
    Ok(())
}

/// Track pod and trigger scheduling.
fn handle_pod_event(state: State, event: PodEvent) {
    match event.event_type {
        EventType::Added => tracing::info!("Added pod"),
        EventType::Deleted => tracing::info!("Deleted pod"),
        EventType::Modified => tracing::info!("Modified pod"),
    }
}
