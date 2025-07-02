use std::sync::Arc;
use serde::de::DeserializeOwned;
use dashmap::{DashMap, DashSet};
use reqwest::Client;
use shared::api::{NodeEvent, PodField, PodPatch};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::io::StreamReader;
use futures_util::TryStreamExt;
use uuid::Uuid;
use rand::seq::IteratorRandom;
use shared::{api::PodEvent, models::{Node, PodObject}};


type State = Arc<SchedulerState>;

pub async fn run() {
    tracing::info!("Initializing scheduler ");
    let (tx, mut rx) = mpsc::channel::<Uuid>(100);
    let state = Arc::new(SchedulerState::new(tx));

    let _ = tokio::spawn(watch_nodes(state.clone()));
    let _ = tokio::spawn(watch_pods(state.clone()));

    tokio::spawn(async move {
        while let Some(pod_id) = rx.recv().await {
            let app_state = state.clone();
            tokio::spawn(async move {
                schedule(app_state, pod_id).await;
            });
        }
    });
}

#[derive(Debug)]
struct SchedulerState {
    nodes: DashMap<String, Node>,
    pods: DashMap<Uuid, PodObject>,
    _pod_map: DashMap<String, DashSet<Uuid>>,
    pod_tx: Sender<Uuid>
}

impl SchedulerState {
    fn new(pod_tx: Sender<Uuid>) -> Self {
        Self { 
            nodes: DashMap::new(),
            pods: DashMap::new(),
            _pod_map: DashMap::new(),
            pod_tx
        }
    }
}

async fn watch_pods(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/pods?nodeName=&watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        state.pods.insert(event.pod.id, event.pod.clone());
        let _ = state.pod_tx.try_send(event.pod.id);
    }).await;
    Ok(())
}

async fn watch_nodes(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/nodes?watch=true".to_string();
    watch_stream::<NodeEvent, _>(&url, move |event| {
        state.nodes.insert(event.node.name.clone(), event.node);
    }).await;
    Ok(())
}


async fn watch_stream<T, F>(url: &str, mut handle_event: F)
where
    T: DeserializeOwned,
    F: FnMut(T) + Send + 'static,
{
    let client = Client::new();
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            tracing::info!("Started watching stream: {}", url);

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<T>(&line) {
                    Ok(event) => handle_event(event),
                    Err(e) => tracing::warn!("Failed to deserialize line: {}\nError: {}", line, e),
                }
            }

            tracing::warn!("Watch stream ended: {}", url);
        }
        Ok(resp) => {
            tracing::error!("Watch request failed: HTTP {}", resp.status());
        }
        Err(err) => {
            tracing::error!("Watch request error: {}", err);
        }
    }
}


async fn schedule(state: State, id: Uuid) {
    let pod = match state.pods.get(&id) {
        Some(p) => p,
        None => {
            tracing::warn!(%id, "Pod not found in state");
            return;
        }
    };

    // Pick a random node
    let node = match state.nodes.iter().choose(&mut rand::rng()) {
        Some(entry) => entry.key().clone(),
        None => {
            tracing::warn!("No available nodes to schedule onto");
            return;
        }
    };

    tracing::info!(
        pod_name=%pod.metadata.user.name,
        node_name=%node,
        "Scheduling pod"
    );

    let patch = PodPatch {
        pod_field: PodField::NodeName,
        value: node.clone(),
    };

    let client = Client::new();
    let url = format!("http://localhost:7620/pods/{}", pod.metadata.user.name);

    match client
        .patch(&url)
        .json(&patch)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {}
        Ok(resp) => {
            tracing::error!(
                status = %resp.status(),
                "Failed to patch pod: non-success response"
            );
        }
        Err(err) => {
            tracing::error!("Failed to patch pod: {}", err);
        }
    }
}
