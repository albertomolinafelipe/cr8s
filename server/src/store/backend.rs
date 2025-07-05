use etcd_client::GetOptions;
use serde::{Serialize, de::DeserializeOwned};
use shared::models::{Node, PodObject};
use uuid::Uuid;

use super::errors::StoreError;

pub struct Backend {
    etcd: etcd_client::Client,
}

impl Backend {
    const POD_PREFIX: &'static str = "/r8s/pods/";
    const NODE_PREFIX: &'static str = "/r8s/nodes/";
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
    fn pod_key(id: &Uuid) -> String {
        format!("{}{}", Self::POD_PREFIX, id)
    }
    fn node_prefix() -> &'static str {
        Self::NODE_PREFIX
    }
    fn node_key(name: &str) -> String {
        format!("{}{}", Self::NODE_PREFIX, name)
    }

    pub async fn get_pod(&self, id: Uuid) -> Result<Option<PodObject>, StoreError> {
        self.get_object::<PodObject>(&Self::pod_key(&id)).await
    }
    pub async fn put_pod(&self, id: &Uuid, pod: &PodObject) -> Result<(), StoreError> {
        self.put_object::<PodObject>(&Self::pod_key(id), pod).await
    }
    pub async fn list_pods(&self) -> Result<Vec<PodObject>, StoreError> {
        self.list_objects::<PodObject>(Self::pod_prefix()).await
    }

    pub async fn get_node(&self, name: &str) -> Result<Option<Node>, StoreError> {
        self.get_object::<Node>(&Self::node_key(name)).await
    }
    pub async fn put_node(&self, name: &str, node: &Node) -> Result<(), StoreError> {
        self.put_object::<Node>(&Self::node_key(name), node).await
    }
    pub async fn list_nodes(&self) -> Result<Vec<Node>, StoreError> {
        self.list_objects::<Node>(Self::node_prefix()).await
    }

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

    async fn list_objects<T>(&self, prefix: &str) -> Result<Vec<T>, StoreError>
    where
        T: DeserializeOwned,
    {
        // pretty rust
        let resp = self
            .etcd
            .clone()
            .get(prefix, Some(GetOptions::new().with_prefix()))
            .await
            .map_err(|error| {
                tracing::error!(%prefix, %error, "Could not list at");
                StoreError::BackendError(error.to_string())
            })?;

        let objs = resp
            .kvs()
            .iter()
            .filter_map(|kv| serde_json::from_str::<T>(kv.value_str().ok()?).ok())
            .collect();

        Ok(objs)
    }
}
