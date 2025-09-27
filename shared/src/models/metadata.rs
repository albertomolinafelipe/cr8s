use std::collections::HashMap;

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
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ObjectMetadata {
    pub name: String,
    #[serde(rename = "ownerReference")]
    pub owner_reference: Option<OwnerReference>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LabelSelector {
    #[serde(rename = "matchLabels")]
    pub match_labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OwnerReference {
    pub id: Uuid,
    pub name: String,
    pub kind: OwnerKind,
    pub controller: bool,
}

#[derive(PartialEq, Debug, Clone, Deserialize, Serialize)]
pub enum OwnerKind {
    ReplicaSet,
}

impl Default for ObjectMetadata {
    fn default() -> Self {
        ObjectMetadata {
            name: Uuid::new_v4().to_string(),
            owner_reference: None,
            labels: HashMap::new(),
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
            generation: 1,
            labels: HashMap::new(),
        }
    }
}

impl From<ObjectMetadata> for Metadata {
    fn from(object: ObjectMetadata) -> Self {
        Metadata {
            name: object.name,
            owner_reference: object.owner_reference,
            labels: object.labels,
            ..Default::default()
        }
    }
}

impl TryFrom<String> for LabelSelector {
    type Error = ();

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let mut labels = HashMap::new();

        for pair in input.split(',') {
            let trimmed = pair.trim();
            if trimmed.is_empty() {
                continue;
            }

            // must contain exactly one '='
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(());
            }

            let key = parts[0].trim();
            let val = parts[1].trim();

            if key.is_empty() || val.is_empty() {
                return Err(());
            }

            // reject duplicate keys
            if labels.contains_key(key) {
                return Err(());
            }

            labels.insert(key.to_string(), val.to_string());
        }

        Ok(LabelSelector {
            match_labels: labels,
        })
    }
}

impl From<LabelSelector> for String {
    fn from(selector: LabelSelector) -> Self {
        selector
            .match_labels
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join(",")
    }
}
