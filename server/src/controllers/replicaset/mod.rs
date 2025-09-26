//! Replicaset controller
//!
//! Watch RSs and its pods to secure intended state

use std::sync::Arc;

use shared::{
    api::{EventType, PodEvent, ReplicaSetEvent},
    models::metadata::OwnerKind,
    utils::watch_stream,
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
                    watch_stream::<PodEvent, _>(
                        &format!("{}?watch=true", rsc.pods_uri),
                        move |event| rsc.handle_pod_event(event),
                    )
                    .await;
                })
            },
            // Watch replicasets
            {
                let rsc = rsc.clone();
                tokio::spawn(async move {
                    watch_stream::<ReplicaSetEvent, _>(
                        &format!("{}?watch=true", rsc.rs_uri),
                        move |event| rsc.handle_replicaset_event(event),
                    )
                    .await;
                })
            },
            // Pull jobs and reconciliate
            {
                let rsc = rsc.clone();
                tokio::spawn(async move {
                    while let Some(rs_id) = rx.recv().await {
                        rsc.reconciliate_task(rs_id);
                    }
                })
            }
        );
    }

    fn reconciliate_task(&self, rs_id: Uuid) {
        let Some(rs) = self.state.get_replicaset(&rs_id) else {
            tracing::error!(id=%rs_id, "Replicaset not state");
            return;
        };

        if rs.spec.replicas <= rs.status.ready_replicas {
            tracing::warn!("RS controller is not done yet");
            return;
        }
        tracing::debug!(%rs_id, "Reconcialiating");
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

    fn handle_pod_event(&self, event: PodEvent) {
        if event
            .pod
            .metadata
            .owner_reference
            .as_ref()
            .map_or(false, |owner| {
                owner.kind == OwnerKind::ReplicaSet && self.state.rs_id_exists(&owner.id)
            })
        {
            match event.event_type {
                EventType::Added => tracing::trace!("RS POD EVENT"),
                EventType::Deleted => { /* TODO */ }
                EventType::Modified => { /* TODO */ }
            };
        }
    }
}
