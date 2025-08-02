//! CLI `delete` command to remove resources from the server by name.
//! Currently supports deleting Pods via HTTP DELETE.

use clap::Parser;
use reqwest::StatusCode;

use crate::{commands::ResourceKind, config::Config};

/// CLI arguments for the `delete` command.
#[derive(Parser, Debug)]
pub struct DeleteArgs {
    /// Type of resource to delete (e.g., Pod)
    #[arg(value_enum)]
    resource: ResourceKind,

    /// Name or ID of the resource
    identifier: String,
}

/// Handles the `delete` command:
/// Constructs a DELETE request based on the resource type and sends it to the server.
#[tokio::main]
pub async fn handle_delete(config: &Config, args: &DeleteArgs) {
    match args.resource {
        ResourceKind::Pod => {
            let url = format!("{}/{}s/{}", &config.url, args.resource, args.identifier);
            match reqwest::Client::new().delete(&url).send().await {
                Ok(resp) => match resp.status() {
                    StatusCode::NO_CONTENT => {}
                    StatusCode::NOT_FOUND => {
                        eprintln!("{} {} not found", args.resource, args.identifier)
                    }
                    _ => eprintln!("Error deleting resource"),
                },
                Err(_) => eprintln!("Error sending request"),
            }
        }
        ResourceKind::Deployment => eprintln!("not implemented"),
    }
}
