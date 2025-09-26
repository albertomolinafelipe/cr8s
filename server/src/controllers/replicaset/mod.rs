//! Replicaset controller
//!
//! Watch RSs and its pods to secure intended state

use std::sync::Arc;

use shared::{
    api::{EventType, PodEvent, ReplicaSetEvent},
    utils::watch_stream,
};
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct RSController {
    pods_uri: String,
    rs_uri: String,
}

impl RSController {
    fn new(apiserver: String) -> (Arc<Self>, mpsc::Receiver<Uuid>) {
        let (_tx, rx) = mpsc::channel::<Uuid>(100);
        (
            Arc::new(Self {
                pods_uri: format!("{}/pods?watch=true", apiserver),
                rs_uri: format!("{}/replicasets?watch=true", apiserver),
            }),
            rx,
        )
    }
    pub async fn run(apiserver: String) {
        tracing::debug!("Running");
        let (rsc, mut _rx) = RSController::new(apiserver);
        let _ = tokio::try_join!(
            // Watch pods
            {
                let rsc = rsc.clone();
                let pods_uri = rsc.pods_uri.clone();
                tokio::spawn(async move {
                    watch_stream::<PodEvent, _>(&pods_uri, move |event| {
                        rsc.handle_pod_event(event)
                    })
                    .await;
                })
            },
            // Watch replicasets
            {
                let rsc = rsc.clone();
                let rs_uri = rsc.rs_uri.clone();
                tokio::spawn(async move {
                    watch_stream::<ReplicaSetEvent, _>(&rs_uri, move |event| {
                        rsc.handle_replicaset_event(event)
                    })
                    .await;
                })
            },
        );
    }

    fn handle_replicaset_event(&self, event: ReplicaSetEvent) {
        match event.event_type {
            EventType::Added => tracing::trace!("RS EVENT"),
            EventType::Deleted => { /* TODO */ }
            EventType::Modified => { /* TODO */ }
        };
    }

    fn handle_pod_event(&self, event: PodEvent) {
        if event.pod.metadata.owner_reference.is_none() {
            tracing::debug!("NON-RS POD EVENT");
            return;
        }
        match event.event_type {
            EventType::Added => tracing::trace!("RS POD EVENT"),
            EventType::Deleted => { /* TODO */ }
            EventType::Modified => { /* TODO */ }
        };
    }
}
