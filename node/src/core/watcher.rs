//! # Assignment Watcher
//!
//! Handles node registration with the control plane and listens for pod assignment
//! events via a streaming HTTP API. It updates local state and dispatches work to the worker
//! subsystem via a channel.

use crate::models::WorkRequest;
use crate::state::State;
use reqwest::Client;
use shared::api::{EventType, NodeRegisterReq, PodEvent};
use shared::utils::watch_stream;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, sleep};

pub async fn run(state: State, tx: Sender<WorkRequest>) -> Result<(), String> {
    register(state.clone()).await?;
    println!("r8s-node ready");
    tracing::debug!("Starting assignment controller");
    let url = format!(
        "{}/pods?watch=true&nodeName={}",
        state.config.server_url, state.config.name
    );
    watch_stream::<PodEvent, _>(&url, move |event| {
        handle_event(state.clone(), event, &tx);
    })
    .await;

    Ok(())
}

/// Registers the node with the control plane server.
async fn register(state: State) -> Result<(), String> {
    let client = Client::new();
    let name = &state.config.name;
    let node_info = NodeRegisterReq {
        port: state.config.port,
        name: state.config.name.clone(),
    };

    for attempt in 1..=state.config.register_retries {
        let response = client
            .post(format!("{}/nodes", state.config.server_url))
            .json(&node_info)
            .send()
            .await;
        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Registered in the system: {}", name);
                return Ok(());
            }
            Ok(resp) => tracing::warn!(
                "Register attempt {} failed: HTTP {}",
                attempt,
                resp.status()
            ),
            Err(err) => tracing::warn!("Register attempt {} failed: {}", attempt, err),
        }

        sleep(Duration::from_secs(2)).await;
    }

    Err("Failed to register".to_string())
}

/// Processes a single pod event by updating local state and forwarding the event to the worker.
fn handle_event(state: State, event: PodEvent, tx: &Sender<WorkRequest>) {
    let req = WorkRequest {
        id: event.pod.metadata.id,
        event: event.event_type.clone(),
    };
    match event.event_type {
        EventType::Modified => state.put_pod(&event.pod),
        EventType::Deleted => state.delete_pod(&event.pod.metadata.id),
        _ => {
            tracing::error!("Unhandled event type: {:?}", event.event_type);
            return;
        }
    }
    if let Err(e) = tx.try_send(req) {
        tracing::error!("Couldn't enqueue pod: {}", e);
    }
}

#[cfg(test)]
mod tests {

    //! - test_modified_event
    //!     only support when its a new pod
    //!     send message, insert pod in system
    //! - test_deleted_event
    //!     send message and delete pod
    //! - test_added_event
    //!     not supported

    use super::*;
    use crate::{docker::test::TestDocker, models::Config, state::new_state_with};
    use shared::{
        api::{EventType, PodEvent},
        models::pod::Pod,
    };
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_modified_event() {
        let docker = Box::new(TestDocker::new());
        let state = new_state_with(Some(Config::default()), Some(docker));
        let pod = Pod::default();

        let (tx, mut rx) = mpsc::channel(1);
        let event = PodEvent {
            pod: pod.clone(),
            event_type: EventType::Modified,
        };

        handle_event(state.clone(), event, &tx);

        let req = rx.recv().await.expect("Should receive a work request");
        assert_eq!(req.id, pod.metadata.id);
        assert_eq!(req.event, EventType::Modified);

        assert!(state.get_pod(&pod.metadata.id).is_some());
    }

    #[tokio::test]
    async fn test_deleted_event() {
        let docker = Box::new(TestDocker::new());
        let state = new_state_with(Some(Config::default()), Some(docker));
        let pod = Pod::default();
        state.put_pod(&pod);

        let (tx, mut rx) = mpsc::channel(1);
        let event = PodEvent {
            pod: pod.clone(),
            event_type: EventType::Deleted,
        };

        handle_event(state.clone(), event, &tx);

        let req = rx.recv().await.expect("Should receive a work request");
        assert_eq!(req.id, pod.metadata.id);
        assert_eq!(req.event, EventType::Deleted);

        assert!(state.get_pod(&pod.metadata.id).is_none());
    }

    #[tokio::test]
    async fn test_added_event() {
        let docker = Box::new(TestDocker::new());
        let state = new_state_with(Some(Config::default()), Some(docker));
        let pod = Pod::default();

        let (tx, mut rx) = mpsc::channel(1);
        let event = PodEvent {
            pod,
            event_type: EventType::Added,
        };

        handle_event(state, event, &tx);

        assert!(rx.try_recv().is_err(), "Added events are not handled");
    }
}
