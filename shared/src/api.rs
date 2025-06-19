use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{PodSpec, UserMetadata};

/// Represents a top-level Kubernetes-like object with metadata and a kind-specific spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpecObject {
    pub metadata: UserMetadata,
    
    #[serde(flatten)]
    pub spec: Spec,
}

/// Enum representing the specification of an object based on its kind.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "spec", rename_all = "PascalCase")]
pub enum Spec {
    Pod(PodSpec),
    Deployment 
}

/// Enum for supported object kinds.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Kind {
    Pod,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct NodeRegisterReq {
    pub port: u16,
    pub name: String,
}


#[derive(Deserialize, Debug)]
pub struct PodQueryParams {
    #[serde(rename = "nodeId")]
    pub node_id: Option<Uuid>,
} 


#[derive(Deserialize, Serialize, Debug)]
pub struct CreateResponse {
    pub id: Uuid,
    pub status: String
} 

impl std::fmt::Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Spec::Pod(_) => write!(f, "pod"),
            Spec::Deployment => write!(f, "deployment")
        }
    }
}
