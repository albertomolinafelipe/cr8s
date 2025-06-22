use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{PodObject, PodSpec, UserMetadata};

#[derive(Deserialize, Serialize, Debug)]
pub struct NodeRegisterReq {
    pub port: u16,
    pub name: String,
}


#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeId")]
    pub node_id: Option<Uuid>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodEvent {
    pub event_type: EventType,
    pub pod: PodObject
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventType {
    ADDED,
    DELETED,
    MODIFIED
}
