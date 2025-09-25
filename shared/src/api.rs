//! Types used for communication between cli, apiserver and nodes
//! including request/response payloads, query params, and event models.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{
    metadata::ObjectMetadata,
    node::Node,
    pod::{ContainerSpec, Pod, PodStatus},
    replicaset::{ReplicaSet, ReplicaSetSpec},
};

// --- Query Params ---

/// Listing or watching pods.
#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeName")]
    pub node_name: Option<String>,
    pub watch: Option<bool>,
}

/// Fetching logs from a container.
#[derive(Deserialize, Debug)]
pub struct LogsQueryParams {
    pub container: Option<String>,
    pub follow: Option<bool>,
}

/// Signal if create comes from controller or cli/user
#[derive(Deserialize, Debug)]
pub struct CreatePodParams {
    pub controller: Option<bool>,
}

// --- Requests and Responses ---

/// Request payload used when registering a node with the server.
#[derive(Deserialize, Serialize, Debug)]
pub struct NodeRegisterReq {
    pub port: u16,
    pub name: String,
}

/// Response returned when a pod or resource is created.
#[derive(Deserialize, Serialize, Debug)]
pub struct CreateResponse {
    pub id: Uuid,
    pub status: String,
}

// --- Manifest ---

/// Definition of a pod to be created, including metadata and spec.
#[derive(Deserialize, Clone, Serialize, Debug, Default)]
pub struct PodManifest {
    pub metadata: ObjectMetadata,
    pub spec: PodContainers,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicaSetManifest {
    pub metadata: ObjectMetadata,
    pub spec: ReplicaSetSpec,
}

#[derive(Deserialize, Clone, Serialize, Debug, Default)]
pub struct PodContainers {
    pub containers: Vec<ContainerSpec>,
}

// --- Pod and Node Events ---

/// Event structure representing changes to a pod.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodEvent {
    pub event_type: EventType,
    pub pod: Pod,
}

/// Event structure representing changes to a node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeEvent {
    pub event_type: EventType,
    pub node: Node,
}

/// Event structure representing changes to a node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicaSetEvent {
    pub event_type: EventType,
    pub replicaset: ReplicaSet,
}

/// Enum representing the type of event that occurred.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum EventType {
    Added,
    Deleted,
    Modified,
}

// --- Patching and Status Updates ---

/// Request to patch a pod field with a new value.
#[derive(Deserialize, Serialize, Debug)]
pub struct PodPatch {
    pub pod_field: PodField,
    pub value: Value,
}

/// Enum representing which field of the pod is being patched.
#[derive(Deserialize, Serialize, Debug)]
pub enum PodField {
    #[serde(rename = "node_name")]
    NodeName,
    #[serde(rename = "spec")]
    Spec,
    #[serde(rename = "status")]
    Status,
}

/// Message used to update the status of a pod and its containers.
#[derive(Deserialize, Serialize, Debug)]
pub struct PodStatusUpdate {
    pub node_name: String,
    pub status: PodStatus,
}
