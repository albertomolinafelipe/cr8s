use clap::Parser;
use erased_serde::serialize_trait_object;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::api::PodManifest;
use shared::models::{PodSpec, UserMetadata};
use tokio::fs;

use crate::config::Config;

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Path to the YAML file containing the deployment spec
    #[clap(short = 'f', long = "file")]
    pub file: String,
}

#[tokio::main]
pub async fn handle_create(config: &Config, args: &CreateArgs) {
    let content = match fs::read_to_string(&args.file).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file '{}': {}", args.file, e);
            return;
        }
    };

    let docs: Vec<GenericManifest> = match serde_yaml::Deserializer::from_str(&content)
        .map(|doc| serde_yaml::from_value(serde_yaml::Value::deserialize(doc).unwrap()))
        .collect::<Result<_, _>>()
    {
        Ok(objs) => objs,
        Err(e) => {
            eprintln!("Failed to parse YAML: {}", e);
            return;
        }
    };

    let client = Client::new();

    for object in docs {
        let url = format!("{}/{}s", config.url, object.spec);
        let manifest = object.spec.into_manifest(object.metadata);

        match client.post(&url).json(&manifest).send().await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Failed to send request to {}: {}", url, e);
                continue;
            }
        };
    }
}

/// Marker trait for serializable manifests
pub trait Manifest: erased_serde::Serialize + Send + Sync {}
impl<T: Serialize + Send + Sync> Manifest for T {}
serialize_trait_object!(Manifest);

/// Represents a top-level Kubernetes-like object with metadata and a kind-specific spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenericManifest {
    pub metadata: UserMetadata,
    #[serde(flatten)]
    pub spec: Spec,
}

/// Enum representing the specification of an object based on its kind.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "spec", rename_all = "PascalCase")]
pub enum Spec {
    Pod(PodSpec),
    Deployment,
}

impl Spec {
    pub fn into_manifest(self, metadata: UserMetadata) -> Box<dyn Manifest> {
        match self {
            Spec::Pod(pod_spec) => Box::new(PodManifest {
                metadata,
                spec: pod_spec,
            }),
            Spec::Deployment => unimplemented!("Deployment support not implemented yet"),
        }
    }
}

impl std::fmt::Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Spec::Pod(_) => write!(f, "pod"),
            Spec::Deployment => write!(f, "deployment"),
        }
    }
}
