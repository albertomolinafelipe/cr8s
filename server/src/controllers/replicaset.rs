//! Replicaset controller
//!
//! Watch RSs and its pods to secure intended state

use shared::{
    api::{EventType, PodEvent, ReplicaSetEvent},
    models::pod::PodPhase,
    utils::watch_stream,
};
use tokio::sync::mpsc;
use uuid::Uuid;

const PODS_URI: &str = "http://localhost:7620/pods?watch=true";
const REPLICASET_URI: &str = "http://localhost:7620/replicasets?watch=true";

pub async fn run() {
    let (tx, mut rx) = mpsc::channel::<Uuid>(100);
    let _ = tokio::spawn(watch_replicasets());
    let _ = tokio::spawn(watch_pods());
}

/// Watch for new nodes in apiserver
async fn watch_pods() -> Result<(), ()> {
    watch_stream::<PodEvent, _>(PODS_URI, move |event| {
        handle_pod_event(event);
    })
    .await;
    Ok(())
}

/// Watch replicasets in apiserver
async fn watch_replicasets() -> Result<(), ()> {
    watch_stream::<ReplicaSetEvent, _>(REPLICASET_URI, move |event| {
        handle_replicaset_event(event);
    })
    .await;
    Ok(())
}

fn handle_replicaset_event(event: ReplicaSetEvent) {
    match event.event_type {
        EventType::Added => tracing::debug!("RS EVENT"),
        EventType::Deleted => { /* TODO */ }
        EventType::Modified => { /* TODO */ }
    };
}

fn handle_pod_event(event: PodEvent) {
    if event.pod.metadata.owner_reference.is_none() {
        tracing::debug!("NON-RS POD EVENT");
        return;
    }
    match event.event_type {
        EventType::Added => tracing::debug!("RS POD EVENT"),
        EventType::Deleted => { /* TODO */ }
        EventType::Modified => { /* TODO */ }
    };
}
