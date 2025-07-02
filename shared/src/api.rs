use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{Node, PodObject, PodSpec, UserMetadata};

#[derive(Deserialize, Serialize, Debug)]
pub struct NodeRegisterReq {
    pub port: u16,
    pub name: String,
}


#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeName")]
    pub node_name: Option<String>,
    pub watch: Option<bool>
} 


#[derive(Deserialize, Serialize, Debug)]
pub struct CreateResponse {
    pub id: Uuid,
    pub status: String
}


#[derive(Deserialize, Serialize, Debug)]
pub struct PodManifest {
    pub metadata: UserMetadata,
    pub spec: PodSpec
} 

// ============================= EVENT

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodEvent {
    pub event_type: EventType,
    pub pod: PodObject
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeEvent {
    pub event_type: EventType,
    pub node: Node
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "ADDED")]
    Added,
    #[serde(rename = "DELETED")]
    Deleted,
    #[serde(rename = "MODIFIED")]
    Modified
}

// ============================= POD PATCH

#[derive(Deserialize, Serialize, Debug)]
pub struct PodPatch {
    pub pod_field: PodField,
    pub value: String
}

#[derive(Deserialize, Serialize, Debug)]
pub enum PodField {
    #[serde(rename = "node_name")]
    NodeName,
    #[serde(rename = "pod_status")]
    PodStatus
}
