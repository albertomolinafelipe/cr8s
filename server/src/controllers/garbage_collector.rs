//! Drift-controller
//! Watch and delete orphan pods

use std::sync::Arc;

use shared::{
    api::{EventType, PodEvent},
    models::pod::PodPhase,
    utils::watch_stream,
};
use tokio::sync::mpsc;

pub struct GCController {
    tx: mpsc::Sender<PodEvent>,
    pods_uri: String,
}

impl GCController {
    fn new(apiserver: String) -> (Arc<Self>, mpsc::Receiver<PodEvent>) {
        let (tx, rx) = mpsc::channel::<PodEvent>(100);
        (
            Arc::new(Self {
                tx,
                pods_uri: format!("{}/pods?watch=true", apiserver),
            }),
            rx,
        )
    }

    pub async fn run(apiserver: String) {
        tracing::debug!("Running");
        let (gc, mut rx) = GCController::new(apiserver);

        let _ = tokio::try_join!(
            // Watch pods
            {
                let gc = gc.clone();
                let pods_uri = gc.pods_uri.clone();
                tokio::spawn(async move {
                    watch_stream::<PodEvent, _>(&pods_uri, move |event| {
                        let _ = gc.tx.try_send(event);
                    })
                    .await;
                })
            },
            // Pull events and reconciliate
            {
                let gc = gc.clone();
                tokio::spawn(async move {
                    while let Some(pod_event) = rx.recv().await {
                        gc.remove_pod(pod_event).await;
                    }
                })
            }
        );
    }

    /// Filter for finished orphan pods
    async fn remove_pod(&self, event: PodEvent) {
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
}
