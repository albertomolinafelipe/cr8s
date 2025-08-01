use crate::{WorkRequest, state::State};
use bollard::secret::ContainerStateStatusEnum;
use shared::api::EventType;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;

pub async fn run(state: State, mut rx: Receiver<WorkRequest>) -> Result<(), String> {
    tracing::info!("Starting reconciliation worker");
    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            let app_state = state.clone();
            tokio::spawn(async move {
                match req.event {
                    EventType::Modified => reconciliate(app_state, req.id).await,
                    EventType::Deleted => delete(app_state, req.id).await,
                    _ => {}
                }
            });
        }
    });
    Ok(())
}

async fn reconciliate(state: State, id: Uuid) {
    let Some(pod) = state.get_pod(&id) else {
        tracing::warn!("Pod {}, not found in pod manager", id);
        return;
    };
    // Check runtime state
    if let Some(_) = state.get_pod_runtime(&pod.id) {
        tracing::error!("Pod already stored in runtime state, not implemented");
        return;
    }

    let runtime = match state.docker_mgr.start_pod(pod).await {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(error=%err, "Failed to start pod");
            return;
        }
    };

    runtime.containers.values().for_each(|c| match c.status {
        ContainerStateStatusEnum::RUNNING
        | ContainerStateStatusEnum::CREATED
        | ContainerStateStatusEnum::EXITED => {}
        _ => {
            tracing::warn!(name=%c.name, "Container didn't start");
        }
    });
    if let Err(msg) = state.add_pod_runtime(runtime) {
        tracing::error!(error=%msg, "Could not add pod runtime to state");
        return;
    }
}

async fn delete(state: State, id: Uuid) {
    let Some(pod_runtime) = state.get_pod_runtime(&id) else {
        return;
    };
    let container_ids: Vec<String> = pod_runtime
        .containers
        .iter()
        .map(|(_, c)| c.id.clone())
        .collect();
    state.delete_pod_runtime(&id);
    match state.docker_mgr.stop_pod(&container_ids).await {
        Ok(()) => {}
        Err(err) => tracing::error!(error=%err, "Failed to delete pod"),
    };
}
