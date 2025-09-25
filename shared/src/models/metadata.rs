use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- Metadata ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
    pub id: Uuid,
    pub name: String,
    #[serde(rename = "ownerReference")]
    pub owner_reference: Option<OwnerReference>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub generation: u16,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ObjectMetadata {
    pub name: String,
    #[serde(rename = "ownerReference")]
    pub owner_reference: Option<OwnerReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OwnerReference {
    pub id: Uuid,
    pub name: String,
    pub kind: OwnerKind,
    pub controller: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum OwnerKind {
    ReplicaSet,
}

impl Default for ObjectMetadata {
    fn default() -> Self {
        ObjectMetadata {
            name: Uuid::new_v4().to_string(),
            owner_reference: None,
        }
    }
}

impl Default for Metadata {
    fn default() -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4();
        Metadata {
            id,
            name: id.to_string(),
            owner_reference: None,
            created_at: now,
            modified_at: now,
            generation: 0,
        }
    }
}

impl From<ObjectMetadata> for Metadata {
    fn from(user: ObjectMetadata) -> Self {
        Metadata {
            name: user.name,
            owner_reference: user.owner_reference,
            ..Default::default()
        }
    }
}
