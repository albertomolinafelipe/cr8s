//! CLI `create` command for applying manifest files to the API server.
//! Supports parsing Kubernetes-like YAML files and sending typed objects over HTTP.

use clap::Parser;
use erased_serde::serialize_trait_object;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::{
    api::{PodContainers, PodManifest, ReplicaSetManifest},
    models::{
        metadata::{LabelSelector, ObjectMetadata},
        pod::ContainerSpec,
        replicaset::ReplicaSetSpec,
    },
};
use tokio::fs;

use crate::config::Config;

/// CLI arguments for the `create` command.
#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Path to the YAML file containing the deployment spec
    #[clap(short = 'f', long = "file")]
    pub file: String,
}

/// Reads a YAML file, parses objects, and posts them to the configured server.
pub async fn handle_create(config: &Config, args: &CreateArgs) {
    // read content of the file
    let content = match fs::read_to_string(&args.file).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file '{}': {}", args.file, e);
            return;
        }
    };

    // parse each object into generic manifests
    // server will serialize into specific manifest depending on endpoint
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

    // send each manifest to the specified resource endpoint
    let client = Client::new();
    for object in docs {
        let url = format!("{}/{}s?controller=false", config.url, object.spec);
        let manifest = object.spec.into_manifest(object.metadata);

        match client.post(&url).json(&manifest).send().await {
            Ok(_) => {}
            Err(err) => eprintln!("Error: {:?}", err),
        };
    }
}

// --- Manifest trait ---

/// Marker trait for serializable manifests that can be sent
pub trait Manifest: erased_serde::Serialize + Send + Sync {}
impl<T: Serialize + Send + Sync> Manifest for T {}
serialize_trait_object!(Manifest);

// --- Generic manifest format ---

/// Represents a top-level Kubernetes-like object with metadata and a kind-specific spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenericManifest {
    pub metadata: ObjectMetadata,
    #[serde(flatten)]
    pub spec: Spec,
}

// --- Supported kinds ---

/// Enum representing the specification of an object based on its `kind` in YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "spec", rename_all = "PascalCase")]
pub enum Spec {
    Pod {
        containers: Vec<ContainerSpec>,
    },
    ReplicaSet {
        replicas: u16,
        selector: LabelSelector,
        template: PodManifest,
    },
}

impl Spec {
    /// Converts the enum variant into a boxed `Manifest` implementation.
    pub fn into_manifest(self, metadata: ObjectMetadata) -> Box<dyn Manifest> {
        match self {
            Spec::Pod { containers } => Box::new(PodManifest {
                metadata,
                spec: PodContainers { containers },
            }),
            Spec::ReplicaSet {
                replicas,
                selector,
                template,
            } => Box::new(ReplicaSetManifest {
                metadata,
                spec: ReplicaSetSpec {
                    replicas,
                    selector,
                    template,
                },
            }),
        }
    }
}

impl std::fmt::Display for Spec {
    /// Formats the spec type as a lowercase kind string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Spec::Pod { .. } => write!(f, "pod"),
            Spec::ReplicaSet { .. } => write!(f, "replicaset"),
        }
    }
}
