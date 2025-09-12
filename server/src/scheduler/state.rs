use dashmap::{DashMap, DashSet};
use shared::models::{node::Node, pod::Pod};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

pub type State = Arc<SchedulerState>;

pub fn new_state(pod_tx: Sender<Uuid>, api_server: Option<String>) -> State {
    Arc::new(SchedulerState::new(pod_tx, api_server))
}

/// In-memory scheduler state shared across tasks.
#[derive(Debug)]
pub struct SchedulerState {
    pub nodes: DashMap<String, Node>,
    pub pods: DashMap<Uuid, Pod>,
    pub pod_map: DashMap<String, DashSet<Uuid>>,
    pub pod_tx: Sender<Uuid>,
    /// optional apiserver attribute for mock test
    /// otherwise we use hardcoded value
    /// should be read from env, but idc
    pub api_server: Option<String>,
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
