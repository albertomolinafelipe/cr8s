use dashmap::DashSet;
use shared::api::{EventType, NodeEvent, PodEvent};
use shared::utils::watch_stream;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::flow::schedule;
use super::state::{State, new_state};

/// One thread watches unassigned pods, another node events
/// They spawn task to schedule the unassigned pods
pub async fn run() {
    tracing::debug!("Initializing");
    let (tx, mut rx) = mpsc::channel::<Uuid>(100);
    let state = new_state(tx, None);

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
    state.pods.insert(event.pod.metadata.id, event.pod.clone());
    // store pod in unassigned group
    state
        .pod_map
        .entry("".to_string())
        .or_insert_with(DashSet::new)
        .insert(event.pod.metadata.id);
    // send pod id to channel for scheduling
    let _ = state.pod_tx.try_send(event.pod.metadata.id);
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

#[cfg(test)]
mod tests {

    //! - test_handle_pod_event_schedule_pod
    //!     ensures a pod is inserted and scheduled upon receiving a pod event.
    //! - test_handle_node_event_schedule_unscheduled_pods
    //!     verifies that unscheduled pods are scheduled when a node is added.

    use super::*;
    use shared::api::EventType;
    use shared::models::{node::Node, pod::Pod};
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn start_mock_server() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path_regex(r"^/pods/.*$"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        server
    }

    #[tokio::test]
    async fn test_handle_pod_event_schedule_pod() {
        // Setup state and mocked patch endpoint
        let (tx, mut rx) = mpsc::channel(10);
        let mock_server = start_mock_server().await;
        let state = new_state(tx, Some(mock_server.uri()));

        let pod = Pod::default();
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
        assert!(state.pods.contains_key(&pod.metadata.id));

        let to_be_scheduled_pod_id = rx.recv().await.expect("Expected pod ID");
        assert_eq!(to_be_scheduled_pod_id, pod.metadata.id);

        schedule(state.clone(), pod.metadata.id).await;

        let node_pods = state.pod_map.get(&node.name);
        assert!(state.nodes.contains_key(&node.name));
        assert!(node_pods.unwrap().contains(&pod.metadata.id));
    }

    #[tokio::test]
    async fn test_handle_node_event_schedule_unscheduled_pods() {
        let (tx, _rx) = mpsc::channel(10);
        let mock_server = start_mock_server().await;
        let state = new_state(tx, Some(mock_server.uri()));

        // Simulate pod being added before any nodes exist
        let pod = Pod::default();
        handle_pod_event(
            state.clone(),
            PodEvent {
                pod: pod.clone(),
                event_type: EventType::Added,
            },
        );

        // Validate pod is marked as unscheduled
        let unscheduled_set = state.pod_map.get("");
        assert!(unscheduled_set.unwrap().contains(&pod.metadata.id));

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
        assert!(node_pods.unwrap().contains(&pod.metadata.id));

        let unscheduled = state.pod_map.get("");
        assert!(!unscheduled.unwrap().contains(&pod.metadata.id));
    }
}
