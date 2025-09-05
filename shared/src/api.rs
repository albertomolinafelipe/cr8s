//! Types used for communication between cli, apiserver and nodes
//! including request/response payloads, query params, and event models.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{
    node::Node,
    pod::{ContainerSpec, Pod, PodPhase},
};

// --- Query Params ---

/// Query parameters for listing or watching pods.
#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeName")]
    pub node_name: Option<String>,
    pub watch: Option<bool>,
}

/// Query parameters for fetching logs from a container.
#[derive(Deserialize, Debug)]
pub struct LogsQueryParams {
    pub container: Option<String>,
    pub follow: Option<bool>,
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

// --- Pod Definitions ---

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct UserMetadata {
    pub name: String,
}

/// Definition of a pod to be created, including metadata and spec.
#[derive(Deserialize, Serialize, Debug, Default)]
pub struct PodManifest {
    pub metadata: UserMetadata,
    pub spec: Vec<ContainerSpec>,
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
    pub status: PodPhase,
    pub container_statuses: Vec<(String, String)>,
}
