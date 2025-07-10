use dashmap::{DashMap, DashSet};
use futures_util::TryStreamExt;
use rand::seq::IteratorRandom;
use reqwest::Client;
use serde::de::DeserializeOwned;
use shared::api::{NodeEvent, PodField, PodPatch};
use shared::{
    api::PodEvent,
    models::{Node, PodObject},
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::io::StreamReader;
use uuid::Uuid;

type State = Arc<SchedulerState>;

pub async fn run() {
    tracing::debug!("Initializing scheduler ");
    let (tx, mut rx) = mpsc::channel::<Uuid>(100);
    let state = Arc::new(SchedulerState::new(tx, None));

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
    pod_map: DashMap<String, DashSet<Uuid>>,
    pod_tx: Sender<Uuid>,
    api_server: Option<String>,
}

impl SchedulerState {
    fn new(pod_tx: Sender<Uuid>, api_server: Option<String>) -> Self {
        Self {
            nodes: DashMap::new(),
            pods: DashMap::new(),
            pod_map: DashMap::new(),
            pod_tx,
            api_server,
        }
    }
}

async fn watch_pods(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/pods?nodeName=&watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        handle_pod_event(state.clone(), event);
    })
    .await;
    Ok(())
}

fn handle_pod_event(state: State, event: PodEvent) {
    state.pods.insert(event.pod.id, event.pod.clone());
    state
        .pod_map
        .entry("".to_string())
        .or_insert_with(DashSet::new)
        .insert(event.pod.id);
    let _ = state.pod_tx.try_send(event.pod.id);
}

async fn watch_nodes(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/nodes?watch=true".to_string();
    watch_stream::<NodeEvent, _>(&url, move |event| {
        handle_node_event(state.clone(), event);
    })
    .await;
    Ok(())
}

fn handle_node_event(state: State, event: NodeEvent) {
    state
        .nodes
        .insert(event.node.name.clone(), event.node.clone());
    if let Some(pods) = state.pod_map.get("") {
        for pod_id in pods.iter() {
            let state = state.clone();
            let pod_id = *pod_id;
            tokio::spawn(async move {
                schedule(state, pod_id).await;
            });
        }
    }
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

            tracing::debug!(url=%url, "Started watching stream");

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<T>(&line) {
                    Ok(event) => handle_event(event),
                    Err(e) => tracing::warn!("Failed to deserialize line: {}\nError: {}", line, e),
                }
            }

            tracing::warn!(url=%url, "Watch stream ended");
        }
        Ok(resp) => {
            tracing::error!(status=%resp.status(), "Watch request failed: HTTP");
        }
        Err(err) => {
            tracing::error!(error=%err, "Watch request error");
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

    let patch = PodPatch {
        pod_field: PodField::NodeName,
        value: node.clone(),
    };

    let client = Client::new();
    let base_url = state
        .api_server
        .as_deref()
        .unwrap_or("http://localhost:7620");
    let url = format!("{}/pods/{}", base_url, pod.metadata.user.name);

    match client.patch(&url).json(&patch).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                pod_name=%pod.metadata.user.name,
                node_name=%node,
                "Scheduled pod"
            );
            state
                .pod_map
                .entry("".to_string())
                .or_insert_with(DashSet::new)
                .remove(&id);
            state
                .pod_map
                .entry(node)
                .or_insert_with(DashSet::new)
                .insert(id);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::api::EventType;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_handle_pod_event_schedule_pod() {
        let (tx, mut rx) = mpsc::channel(10);

        let mock_server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path_regex(r"^/pods/.*$"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = Arc::new(SchedulerState::new(tx, Some(mock_server.uri())));

        let pod = PodObject::default();

        let node = Node::default();
        let node_event = NodeEvent {
            node: node.clone(),
            event_type: EventType::Added,
        };
        handle_node_event(state.clone(), node_event);

        let event = PodEvent {
            pod: pod.clone(),
            event_type: EventType::Added,
        };
        handle_pod_event(state.clone(), event);

        // Check pod stored in state
        assert!(state.pods.contains_key(&pod.id));
        // Check its in the channel
        let scheduled_pod_id = rx.recv().await.expect("Expected pod ID");
        assert_eq!(scheduled_pod_id, pod.id);
        schedule(state.clone(), pod.id).await;
        let node_pods = state.pod_map.get(&node.name);
        assert!(state.nodes.contains_key(&node.name));
        assert!(node_pods.is_some());
        assert!(node_pods.unwrap().contains(&pod.id));
    }

    #[tokio::test]
    async fn test_handle_node_event_schedule_unscheduled_pods() {
        let (tx, _rx) = mpsc::channel(10);

        let mock_server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path_regex(r"^/pods/.*$"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let state = Arc::new(SchedulerState::new(tx, Some(mock_server.uri())));

        let pod = PodObject::default();
        let event = PodEvent {
            pod: pod.clone(),
            event_type: EventType::Added,
        };
        handle_pod_event(state.clone(), event);

        // Check pod is unscheduled
        let unscheduled_set = state.pod_map.get("");
        assert!(unscheduled_set.is_some());
        assert!(unscheduled_set.unwrap().contains(&pod.id));

        // Send event
        let node = Node::default();
        let node_event = NodeEvent {
            node: node.clone(),
            event_type: EventType::Added,
        };
        handle_node_event(state.clone(), node_event);

        // Wait a little bit
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Pod should now be assigned
        let node_pods = state.pod_map.get(&node.name);
        assert!(state.nodes.contains_key(&node.name));
        assert!(node_pods.is_some());
        assert!(node_pods.unwrap().contains(&pod.id));

        // Pod should no longer be in the unscheduled pod set
        let unscheduled_pods = state.pod_map.get("");
        if let Some(set) = unscheduled_pods {
            assert!(!set.contains(&pod.id));
        }
    }
}
