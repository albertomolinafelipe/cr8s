//! Replicaset controller
//!
//! Watch RSs and its pods to secure intended state

use shared::{
    api::{EventType, PodEvent, ReplicaSetEvent},
    models::pod::PodPhase,
    utils::watch_stream,
};

pub async fn run() {
    watch_replicasets().await.expect(".")
}

async fn watch_replicasets() -> Result<(), ()> {
    let url = "http://localhost:7620/replicasets?watch=true".to_string();
    watch_stream::<ReplicaSetEvent, _>(&url, move |event| {
        tokio::spawn(async move {
            handle_replicaset_event(event).await;
        });
    })
    .await;
    Ok(())
}

/// Track pod and trigger scheduling.
async fn handle_replicaset_event(event: ReplicaSetEvent) {
    tracing::debug!("Got a RS event");
}
