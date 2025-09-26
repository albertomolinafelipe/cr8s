use std::sync::Arc;

use dashmap::DashMap;
use shared::models::replicaset::ReplicaSet;
use uuid::Uuid;

pub type State = Arc<RSState>;

#[derive(Debug)]
pub struct RSState {
    rs: DashMap<Uuid, ReplicaSet>,
}

impl RSState {
    pub fn new() -> State {
        Arc::new(Self { rs: DashMap::new() })
    }

    pub fn rs_id_exists(&self, id: &Uuid) -> bool {
        self.rs.contains_key(id)
    }

    pub fn add_replicaset(&self, rs: &ReplicaSet) {
        if self.rs.contains_key(&rs.metadata.id) {
            return;
        }
        self.rs.insert(rs.metadata.id, rs.clone());
    }

    pub fn get_replicaset(&self, id: &Uuid) -> Option<ReplicaSet> {
        self.rs.get(id).map(|entry| entry.clone())
    }
}
