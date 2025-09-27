//! Replicaset controller
//!
//! Watch RSs and its pods to secure intended state

use std::sync::Arc;

use reqwest::Client;
use shared::{
    api::{EventType, PodEvent, PodManifest, ReplicaSetEvent},
    models::{metadata::OwnerKind, pod::Pod},
    utils::{watch_stream, watch_stream_async},
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::controllers::replicaset::state::{RSState, State};

mod state;

pub struct RSController {
    state: State,
    pods_uri: String,
    rs_uri: String,
    tx: mpsc::Sender<Uuid>,
}

impl RSController {
    fn new(apiserver: String) -> (Arc<Self>, mpsc::Receiver<Uuid>) {
        let (tx, rx) = mpsc::channel::<Uuid>(100);
        (
            Arc::new(Self {
                state: RSState::new(),
                tx,
                pods_uri: format!("{}/pods", apiserver),
                rs_uri: format!("{}/replicasets", apiserver),
            }),
            rx,
        )
    }
    pub async fn run(apiserver: String) {
        tracing::debug!("Running");
        let (rsc, mut rx) = RSController::new(apiserver);
        let _ = tokio::try_join!(
            // Watch pods
            {
                let rsc = rsc.clone();
                tokio::spawn(async move {
                    watch_stream_async(&format!("{}?watch=true", rsc.pods_uri), move |event| {
                        let rsc = rsc.clone();
                        async move {
                            rsc.handle_pod_event(event).await;
                        }
                    })
                    .await;
                })
            },
            // Watch replicasets
            {
                let rsc = rsc.clone();
                tokio::spawn(async move {
                    watch_stream(&format!("{}?watch=true", rsc.rs_uri), move |event| {
                        rsc.handle_replicaset_event(event)
                    })
                    .await;
                })
            },
            // Pull jobs and reconciliate
            {
                let rsc = rsc.clone();
                tokio::spawn(async move {
                    while let Some(rs_id) = rx.recv().await {
                        rsc.reconciliate_task(rs_id).await;
                    }
                })
            }
        );
    }

    async fn reconciliate_task(&self, rs_id: Uuid) {
        let Some(rs) = self.state.get_replicaset(&rs_id) else {
            tracing::error!(id=%rs_id, "Replicaset not state");
            return;
        };

        if rs.spec.replicas <= rs.status.ready_replicas {
            tracing::warn!("RS controller is not done yet");
            return;
        }

        // Create pods
        let client = Client::new();
        let url = format!("{}?controller=true", self.pods_uri);
        for _ in 0..(rs.spec.replicas - rs.status.ready_replicas) {
            // regenerate manifest if 409?
            let manifest: PodManifest = rs.clone().into();
            match client.post(&url).json(&manifest).send().await {
                Ok(resp) if resp.status().is_success() => tracing::debug!("Created RS pod"),
                Ok(resp) => tracing::error!("Failed to create pod: {}", resp.status()),
                Err(err) => tracing::error!("Failed to create pod: {}", err),
            }
        }
    }

    fn handle_replicaset_event(&self, event: ReplicaSetEvent) {
        match event.event_type {
            EventType::Deleted => {
                /* TODO */
                return;
            }
            EventType::Modified => {
                /* TODO */
                return;
            }
            EventType::Added => self.state.add_replicaset(&event.replicaset),
        };
        let _ = self.tx.try_send(event.replicaset.metadata.id);
    }

    async fn handle_pod_event(&self, event: PodEvent) {
        if event
            .pod
            .metadata
            .owner_reference
            .as_ref()
            .map_or(false, |owner| {
                owner.kind == OwnerKind::ReplicaSet && self.state.rs_id_exists(&owner.id)
            })
        {
            let client = Client::new();
            let rs_id = event.pod.metadata.owner_reference.unwrap().id;
            let Some(rs) = self.state.get_replicaset(&rs_id) else {
                tracing::error!(id=%rs_id, "Replicaset not in state");
                return;
            };
            let param: String = rs.spec.selector.into();
            let url = format!("{}?labelSelector={}", self.pods_uri, param);
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let Ok(pods) = resp.json::<Vec<Pod>>().await else {
                        tracing::error!("Couldnt parse pods");
                        return;
                    };
                    tracing::debug!(len=%pods.len(), "Received");
                }
                Ok(resp) => tracing::error!("Failed to get pods: {}", resp.status()),
                Err(err) => tracing::error!("Failed to get pods: {}", err),
            }
        }
    }
}
