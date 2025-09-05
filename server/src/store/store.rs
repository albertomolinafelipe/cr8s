//! Etcd-backed implementation of the `Store` trait.
//!
//! This module provides basic CRUD operations for Pods and Nodes using etcd
//! as the backend. It serializes and deserializes objects using JSON and
//! manages key construction using standard prefixes.

use etcd_client::GetOptions;
use serde::{Serialize, de::DeserializeOwned};
use shared::models::{node::Node, pod::Pod};
use uuid::Uuid;

use super::errors::StoreError;

use async_trait::async_trait;

/// Trait for persistent store functionality (e.g., etcd, memory).
#[async_trait]
pub trait Store: Send + Sync {
    async fn get_pod(&self, id: Uuid) -> Result<Option<Pod>, StoreError>;
    async fn put_pod(&self, id: &Uuid, pod: &Pod) -> Result<(), StoreError>;
    async fn list_pods(&self) -> Result<Vec<Pod>, StoreError>;
    async fn delete_pod(&self, id: &Uuid) -> Result<(), StoreError>;

    async fn get_node(&self, name: &str) -> Result<Option<Node>, StoreError>;
    async fn put_node(&self, name: &str, node: &Node) -> Result<(), StoreError>;
    async fn list_nodes(&self) -> Result<Vec<Node>, StoreError>;
}

/// Etcd-backed store for persisting cluster state
pub struct EtcdStore {
    etcd: etcd_client::Client,
}

impl EtcdStore {
    const POD_PREFIX: &'static str = "/r8s/pods/";
    const NODE_PREFIX: &'static str = "/r8s/nodes/";

    /// Creates a new EtcdStore instance, connecting to the ETCD_ADDR environment variable.
    pub async fn new() -> Self {
        let etcd_addr =
            std::env::var("ETCD_ADDR").unwrap_or_else(|_| "http://etcd:2379".to_string());
        tracing::info!(%etcd_addr, "Connecting to backend ");

        let etcd = etcd_client::Client::connect([&etcd_addr], None)
            .await
            .expect("Failed to connect to etcd");
        Self { etcd }
    }
    fn pod_prefix() -> &'static str {
        Self::POD_PREFIX
    }
    fn node_prefix() -> &'static str {
        Self::NODE_PREFIX
    }
    fn pod_key(id: &Uuid) -> String {
        format!("{}{}", Self::POD_PREFIX, id)
    }
    fn node_key(name: &str) -> String {
        format!("{}{}", Self::NODE_PREFIX, name)
    }

    /// Deletes an object from etcd by key.
    async fn delete_object(&self, key: &str) -> Result<(), StoreError> {
        self.etcd.clone().delete(key, None).await.map_err(|e| {
            tracing::error!(%key, %e, "Failed to delete key");
            StoreError::BackendError(e.to_string())
        })?;
        Ok(())
    }

    /// Retrieves a single object from etcd and deserializes it.
    async fn get_object<T>(&self, key: &str) -> Result<Option<T>, StoreError>
    where
        T: DeserializeOwned,
    {
        // pretty rust
        self.etcd
            .clone()
            .get(key, None)
            .await
            .map_err(|error| {
                tracing::error!(%key, %error, "Could not get at");
                StoreError::BackendError(error.to_string())
            })?
            .kvs()
            .first()
            .map(|kv| {
                let val = kv
                    .value_str()
                    .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;

                serde_json::from_str::<T>(val)
                    .map_err(|e| StoreError::UnexpectedError(e.to_string()))
            })
            .transpose()
    }

    /// Serializes and writes an object to etcd
    async fn put_object<T>(&self, key: &str, value: &T) -> Result<(), StoreError>
    where
        T: Serialize,
    {
        let json =
            serde_json::to_string(value).map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        self.etcd
            .clone()
            .put(key, json, None)
            .await
            .map_err(|e| StoreError::UnexpectedError(e.to_string()))?;
        Ok(())
    }

    /// Lists all objects stored under a given prefix.
    async fn list_objects<T>(&self, prefix: &str) -> Result<Vec<T>, StoreError>
    where
        T: DeserializeOwned,
    {
        // pretty rust
        Ok(self
            .etcd
            .clone()
            .get(prefix, Some(GetOptions::new().with_prefix()))
            .await
            .map_err(|error| {
                tracing::error!(%prefix, %error, "Could not list at");
                StoreError::BackendError(error.to_string())
            })?
            .kvs()
            .iter()
            .filter_map(|kv| serde_json::from_str::<T>(kv.value_str().ok()?).ok())
            .collect())
    }
}

#[async_trait]
impl Store for EtcdStore {
    async fn get_pod(&self, id: Uuid) -> Result<Option<Pod>, StoreError> {
        self.get_object::<Pod>(&Self::pod_key(&id)).await
    }
    async fn put_pod(&self, id: &Uuid, pod: &Pod) -> Result<(), StoreError> {
        self.put_object::<Pod>(&Self::pod_key(id), pod).await
    }
    async fn list_pods(&self) -> Result<Vec<Pod>, StoreError> {
        self.list_objects::<Pod>(Self::pod_prefix()).await
    }

    async fn get_node(&self, name: &str) -> Result<Option<Node>, StoreError> {
        self.get_object::<Node>(&Self::node_key(name)).await
    }
    async fn put_node(&self, name: &str, node: &Node) -> Result<(), StoreError> {
        self.put_object::<Node>(&Self::node_key(name), node).await
    }
    async fn list_nodes(&self) -> Result<Vec<Node>, StoreError> {
        self.list_objects::<Node>(Self::node_prefix()).await
    }
    async fn delete_pod(&self, id: &Uuid) -> Result<(), StoreError> {
        self.delete_object(&Self::pod_key(id)).await
    }
}
