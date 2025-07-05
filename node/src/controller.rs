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
use uuid::Uuid;

pub async fn run(state: State, tx: Sender<Uuid>) -> Result<(), String> {
    register(state.clone()).await?;
    println!("r8s-node ready");
    tracing::debug!("Starting assignment controller");
    watch(state.clone(), &tx).await?;

    Ok(())
}

async fn watch(state: State, tx: &Sender<Uuid>) -> Result<(), String> {
    let client = Client::new();

    let url = format!(
        "{}/pods?nodeName={}&watch=true",
        state.config.server_url,
        state.node_name()
    );

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            tracing::debug!(node_name=%state.node_name(), "Started watching pod assignments for");

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PodEvent>(&line) {
                    Ok(event) => handle_event(state.clone(), event, tx).await,
                    Err(e) => {
                        tracing::warn!(line=%line, error=%e, "Failed to deserialize");
                    }
                }
            }
            tracing::warn!("Watch stream ended.");
        }
        Ok(resp) => {
            tracing::error!("Watch request failed: HTTP {}", resp.status());
        }
        Err(err) => {
            tracing::error!("Watch request error: {}", err);
        }
    }

    Ok(())
}

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
                state.set_name(name.to_string());
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

async fn handle_event(state: State, event: PodEvent, tx: &Sender<Uuid>) {
    match event.event_type {
        EventType::Modified => {
            if let Err(e) = state.add_pod(&event.pod) {
                tracing::error!("Couldn't add pod: {}", e);
            } else if let Err(e) = tx.send(event.pod.id).await {
                tracing::error!("Couldn't enqueue pod: {}", e);
            }
        }
        _ => {
            tracing::warn!("Unhandled event type: {:?}", event.event_type);
        }
    }
}
