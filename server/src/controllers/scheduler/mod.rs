mod filter;
mod flow;
mod scorer;
mod state;

use std::sync::Arc;

use shared::api::{EventType, NodeEvent, PodEvent};
use shared::utils::watch_stream;
use tokio::sync::mpsc;
use uuid::Uuid;

use flow::SchedulerFlow;
use state::{SchedulerState, State};

pub struct Scheduler {
    state: State,
    tx: mpsc::Sender<Uuid>,
    pods_uri: String,
    nodes_uri: String,
}

impl Scheduler {
    fn new(apiserver: String) -> (Arc<Self>, mpsc::Receiver<Uuid>) {
        let (tx, rx) = mpsc::channel::<Uuid>(100);
        (
            Arc::new(Self {
                state: SchedulerState::new(&apiserver),
                tx,
                pods_uri: format!("{}/pods?watch=true", apiserver),
                nodes_uri: format!("{}/nodes?watch=true", apiserver),
            }),
            rx,
        )
    }

    pub async fn run(apiserver: String) {
        tracing::debug!("Running");
        let (sched, mut rx) = Scheduler::new(apiserver);

        let _ = tokio::try_join!(
            // Watch nodes
            {
                let sched = sched.clone();
                let nodes_uri = sched.nodes_uri.clone();
                tokio::spawn(async move {
                    watch_stream(&nodes_uri, move |event| {
                        sched.handle_node_event(event);
                    })
                    .await;
                })
            },
            // Watch pods
            {
                let sched = sched.clone();
                let pods_uri = sched.pods_uri.clone();
                tokio::spawn(async move {
                    watch_stream(&pods_uri, move |event| {
                        sched.handle_pod_event(event);
                    })
                    .await;
                })
            },
            // Pull jobs and schedule pods
            {
                let sched = sched.clone();
                tokio::spawn(async move {
                    while let Some(pod_id) = rx.recv().await {
                        sched.schedule(pod_id).await;
                    }
                })
            }
        );
    }

    async fn schedule(&self, id: Uuid) {
        let pod = match self.state.pods.get(&id) {
            Some(p) => p.clone(),
            None => {
                tracing::warn!(%id, "Pod not found in state");
                return;
            }
        };

        let flow = SchedulerFlow::new(&self.state, pod, None, None)
            .execute()
            .await;

        if let (true, Some(node)) = (flow.accepted, &flow.chosen) {
            self.state.assign_pod(&id, node);
        } else {
            tracing::error!("Could not schedule pod");
        }
    }

    fn handle_pod_event(&self, event: PodEvent) {
        match event.event_type {
            EventType::Added => {
                if event.pod.spec.node_name.is_empty() {
                    self.state.add_pod(&event.pod);
                    let _ = self.tx.try_send(event.pod.metadata.id);
                }
            }
            EventType::Deleted => self.state.delete_pod(&event.pod.metadata.id),
            EventType::Modified => { /*TODO*/ }
        }
    }

    fn handle_node_event(&self, event: NodeEvent) {
        if event.event_type != EventType::Added {
            tracing::warn!("Scheduler only implements `Add` node events");
            return;
        }
        self.state.add_node(&event.node);
        if let Some(pods) = self.state.pod_map.get("") {
            for pod_id in pods.iter() {
                let res = self.tx.try_send(*pod_id);
                tracing::debug!("RES: {res:?}");
            }
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
        let mock_server = start_mock_server().await;
        let (sched, mut rx) = Scheduler::new(mock_server.uri());

        let pod = Pod::default();
        let node = Node::default();

        // Simulate node and pod event
        sched.handle_node_event(NodeEvent {
            node: node.clone(),
            event_type: EventType::Added,
        });

        sched.handle_pod_event(PodEvent {
            pod: pod.clone(),
            event_type: EventType::Added,
        });

        // Verify pod is queued and eventually scheduled
        assert!(sched.state.pods.contains_key(&pod.metadata.id));

        let to_be_scheduled_pod_id = rx.recv().await.expect("Expected pod ID");
        assert_eq!(to_be_scheduled_pod_id, pod.metadata.id);

        sched.schedule(pod.metadata.id).await;

        let node_pods = sched.state.pod_map.get(&node.name);
        assert!(sched.state.nodes.contains_key(&node.name));
        assert!(node_pods.unwrap().contains(&pod.metadata.id));
    }

    #[tokio::test]
    async fn test_handle_node_event_schedule_unscheduled_pods() {
        let mock_server = start_mock_server().await;
        let (sched, mut rx) = Scheduler::new(mock_server.uri());

        // Simulate pod being added before any nodes exist
        let pod = Pod::default();
        sched.handle_pod_event(PodEvent {
            pod: pod.clone(),
            event_type: EventType::Added,
        });
        // replicate worker
        {
            let sched = sched.clone();
            tokio::spawn(async move {
                while let Some(pod_id) = rx.recv().await {
                    sched.schedule(pod_id).await;
                }
            });
        }

        // Validate pod is marked as unscheduled
        let unscheduled_set = sched.state.pod_map.get("");
        assert!(unscheduled_set.unwrap().contains(&pod.metadata.id));

        // Add node and verify scheduling occurs
        let node = Node::default();
        sched.handle_node_event(NodeEvent {
            node: node.clone(),
            event_type: EventType::Added,
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let node_pods = sched.state.pod_map.get(&node.name);
        assert!(node_pods.unwrap().contains(&pod.metadata.id));

        let unscheduled = sched.state.pod_map.get("");
        assert!(!unscheduled.unwrap().contains(&pod.metadata.id));
    }
}
