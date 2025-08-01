use super::errors::StoreError;
use super::store::Store;
use async_trait::async_trait;
use dashmap::DashMap;
use shared::models::{Node, PodObject};
use uuid::Uuid;

pub struct TestStore {
    pub pods: DashMap<Uuid, PodObject>,
    pub nodes: DashMap<String, Node>,
}

impl TestStore {
    pub fn new() -> Self {
        Self {
            pods: DashMap::new(),
            nodes: DashMap::new(),
        }
    }
}

#[async_trait]
impl Store for TestStore {
    async fn get_pod(&self, id: Uuid) -> Result<Option<PodObject>, StoreError> {
        Ok(self.pods.get(&id).map(|ref_entry| ref_entry.clone()))
    }

    async fn put_pod(&self, id: &Uuid, pod: &PodObject) -> Result<(), StoreError> {
        self.pods.insert(*id, pod.clone());
        Ok(())
    }

    async fn list_pods(&self) -> Result<Vec<PodObject>, StoreError> {
        Ok(self
            .pods
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn delete_pod(&self, id: &Uuid) -> Result<(), StoreError> {
        self.pods.remove(id);
        Ok(())
    }

    async fn get_node(&self, name: &str) -> Result<Option<Node>, StoreError> {
        Ok(self.nodes.get(name).map(|ref_entry| ref_entry.clone()))
    }

    async fn put_node(&self, name: &str, node: &Node) -> Result<(), StoreError> {
        self.nodes.insert(name.to_string(), node.clone());
        Ok(())
    }

    async fn list_nodes(&self) -> Result<Vec<Node>, StoreError> {
        Ok(self
            .nodes
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }
}
