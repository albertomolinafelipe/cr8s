//! r8s-scheduler
//! Watches pods and nodes, then assigns unscheduled pods to available nodes.
//! Completely separate from other components

use dashmap::{DashMap, DashSet};
use rand::seq::IteratorRandom;
use reqwest::Client;
use serde_json::Value;
use shared::api::{EventType, NodeEvent, PodEvent, PodField, PodPatch};
use shared::models::{Node, PodObject};
use shared::utils::watch_stream;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};
use uuid::Uuid;

type State = Arc<SchedulerState>;

pub async fn run() {
    tracing::debug!("Initializing scheduler ");
    let (tx, mut rx) = mpsc::channel::<Uuid>(100);
    let state = Arc::new(SchedulerState::new(tx, None));

    // Watch nodes and pods in the background
    let _ = tokio::spawn(watch_nodes(state.clone()));
    let _ = tokio::spawn(watch_pods(state.clone()));

    // Handle scheduling of pods via channel
    tokio::spawn(async move {
        while let Some(pod_id) = rx.recv().await {
            let app_state = state.clone();
            tokio::spawn(async move {
                schedule(app_state, pod_id).await;
            });
        }
    });
}

/// In-memory scheduler state shared across tasks.
#[derive(Debug)]
struct SchedulerState {
    nodes: DashMap<String, Node>,
    pods: DashMap<Uuid, PodObject>,
    pod_map: DashMap<String, DashSet<Uuid>>,
    pod_tx: Sender<Uuid>,
    /// optional apiserver attribute for mock test
    /// otherwise we use hardcoded value
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

/// Watch for new nodes in apiserver
async fn watch_pods(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/pods?nodeName=&watch=true".to_string();
    watch_stream::<PodEvent, _>(&url, move |event| {
        handle_pod_event(state.clone(), event);
    })
    .await;
    Ok(())
}

/// Watch pods in apiserver
async fn watch_nodes(state: State) -> Result<(), ()> {
    let url = "http://localhost:7620/nodes?watch=true".to_string();
    watch_stream::<NodeEvent, _>(&url, move |event| {
        handle_node_event(state.clone(), event);
    })
    .await;
    Ok(())
}

/// Track pod and trigger scheduling.
fn handle_pod_event(state: State, event: PodEvent) {
    if event.event_type != EventType::Added {
        tracing::error!("Scheduler only implemented new pods");
        return;
    }
    // add pod to map
    state.pods.insert(event.pod.id, event.pod.clone());
    // store pod in unassigned group
    state
        .pod_map
        .entry("".to_string())
        .or_insert_with(DashSet::new)
        .insert(event.pod.id);
    // send pod id to channel for scheduling
    let _ = state.pod_tx.try_send(event.pod.id);
}

/// Handle node event: track node and attempt to schedule all unscheduled pods.
fn handle_node_event(state: State, event: NodeEvent) {
    // Insert new node
    state
        .nodes
        .insert(event.node.name.clone(), event.node.clone());
    // If there are unscheduled pods run scheduling loop
    // This could happen if no nodes where registered when those pods where created
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

/// Assigns a pod to a random available node by patching the API server.
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

    // make patch call to api server
    let patch = PodPatch {
        pod_field: PodField::NodeName,
        value: Value::String(node.clone()),
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
            // Move pod to its assigned node group
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

    //! - test_handle_pod_event_schedule_pod
    //!     ensures a pod is inserted and scheduled upon receiving a pod event.
    //! - test_handle_node_event_schedule_unscheduled_pods
    //!     verifies that unscheduled pods are scheduled when a node is added.

    use super::*;
    use shared::api::EventType;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_handle_pod_event_schedule_pod() {
        // Setup state and mocked patch endpoint
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

        // Simulate node and pod event
        handle_node_event(
            state.clone(),
            NodeEvent {
                node: node.clone(),
                event_type: EventType::Added,
            },
        );

        handle_pod_event(
            state.clone(),
            PodEvent {
                pod: pod.clone(),
                event_type: EventType::Added,
            },
        );

        // Verify pod is queued and eventually scheduled
        assert!(state.pods.contains_key(&pod.id));
        let scheduled_pod_id = rx.recv().await.expect("Expected pod ID");
        assert_eq!(scheduled_pod_id, pod.id);
        schedule(state.clone(), pod.id).await;
        let node_pods = state.pod_map.get(&node.name);
        assert!(state.nodes.contains_key(&node.name));
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

        // Simulate pod being added before any nodes exist
        let pod = PodObject::default();
        handle_pod_event(
            state.clone(),
            PodEvent {
                pod: pod.clone(),
                event_type: EventType::Added,
            },
        );

        // Validate pod is marked as unscheduled
        let unscheduled_set = state.pod_map.get("");
        assert!(unscheduled_set.unwrap().contains(&pod.id));

        // Add node and verify scheduling occurs
        let node = Node::default();
        handle_node_event(
            state.clone(),
            NodeEvent {
                node: node.clone(),
                event_type: EventType::Added,
            },
        );

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let node_pods = state.pod_map.get(&node.name);
        assert!(node_pods.unwrap().contains(&pod.id));

        let unscheduled = state.pod_map.get("");
        assert!(!unscheduled.unwrap().contains(&pod.id));
    }
}
