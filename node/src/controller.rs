//! # Assignment Controller
//!
//! Handles node registration with the control plane and listens for pod assignment
//! events via a streaming HTTP API. It updates local state and dispatches work to the worker
//! subsystem via a channel.

use crate::WorkRequest;
use crate::state::State;
use futures_util::TryStreamExt;
use reqwest::Client;
use shared::api::{EventType, NodeRegisterReq, PodEvent};
use tokio::time::{Duration, sleep};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::Sender,
};

use tokio_util::io::StreamReader;

pub async fn run(state: State, tx: Sender<WorkRequest>) -> Result<(), String> {
    register(state.clone()).await?;
    println!("r8s-node ready");
    tracing::debug!("Starting assignment controller");
    watch(state.clone(), &tx).await?;

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

/// Watches the control plane for pod assignment changes using a streaming API.
///
/// Sends each relevant event to the worker subsystem via `tx`.
async fn watch(state: State, tx: &Sender<WorkRequest>) -> Result<(), String> {
    let client = Client::new();

    let url = format!(
        "{}/pods?nodeName={}&watch=true",
        state.config.server_url, state.config.name
    );

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            tracing::debug!("Started watching pod assignments");

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PodEvent>(&line) {
                    Ok(event) => handle_event(state.clone(), event, tx).await,
                    Err(e) => tracing::warn!(line=%line, error=%e, "Failed to deserialize"),
                }
            }
            tracing::warn!("Watch stream ended");
        }
        Ok(resp) => tracing::error!("Watch request failed: HTTP {}", resp.status()),
        Err(err) => tracing::error!("Watch request error: {}", err),
    }

    Ok(())
}

/// Processes a single pod event by updating local state and forwarding the event to the worker.
///
/// Only `Modified` and `Deleted` events are handled. Other event types are logged as errors.
async fn handle_event(state: State, event: PodEvent, tx: &Sender<WorkRequest>) {
    let req = WorkRequest {
        id: event.pod.id,
        event: event.event_type.clone(),
    };
    match event.event_type {
        EventType::Modified => state.put_pod(&event.pod),
        EventType::Deleted => state.delete_pod(&event.pod.id),
        _ => {
            tracing::error!("Unhandled event type: {:?}", event.event_type);
            return;
        }
    }
    if let Err(e) = tx.send(req).await {
        tracing::error!("Couldn't enqueue pod: {}", e);
    }
}
