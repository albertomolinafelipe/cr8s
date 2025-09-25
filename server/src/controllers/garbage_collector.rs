//! Drift-controller
//! Watch and delete orphan pods

use shared::{
    api::{EventType, PodEvent},
    models::pod::PodPhase,
    utils::watch_stream,
};

pub async fn run() {
    watch_pods().await.expect(".")
}

async fn watch_pods() -> Result<(), ()> {
    let url = "http://localhost:7620/pods?watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        tokio::spawn(async move {
            handle_pod_event(event).await;
        });
    })
    .await;
    Ok(())
}

/// Filter for finished orphan pods
async fn handle_pod_event(event: PodEvent) {
    if event.pod.metadata.owner_reference.is_some() {
        tracing::trace!(pod=%event.pod.metadata.name, "Pod with owner, skipping");
        return;
    }
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
