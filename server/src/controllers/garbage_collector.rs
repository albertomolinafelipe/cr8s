//! Drift-controller
//! Watch and delete broken pods

use std::sync::Arc;

use dashmap::DashMap;
use shared::{
    api::{EventType, PodEvent},
    models::{PodObject, PodStatus},
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
    _pods: DashMap<Uuid, PodObject>,
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
        EventType::Added => tracing::trace!("Added pod"),
        EventType::Deleted => tracing::trace!("Deleted pod"),
        EventType::Modified => {
            tracing::trace!(status=%event.pod.pod_status, "Modified pod");
            match event.pod.pod_status {
                PodStatus::Failed | PodStatus::Succeeded => {
                    let pod_id = event.pod.metadata.user.name;
                    let url = format!("http://localhost:7620/pods/{}", pod_id);

                    tracing::info!("Deleting pod {} at {}", pod_id, url);

                    match reqwest::Client::new().delete(&url).send().await {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            tracing::info!(
                                "Delete response for pod {}: {} - {}",
                                pod_id,
                                status,
                                body
                            );
                        }
                        Err(err) => {
                            tracing::error!("Failed to delete pod {}: {}", pod_id, err);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
