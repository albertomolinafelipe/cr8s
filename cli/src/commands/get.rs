//! CLI `get` command to retrieve and display resources (nodes, pods) from the server.
//! Fetches a list and displays it as a formatted table.

use clap::Parser;
use shared::models::{node::Node, pod::Pod, replicaset::ReplicaSet};
use tabled::{Table, settings::Style};

use super::ResourceType;
use crate::config::Config;

/// CLI arguments for the `get` command.
#[derive(Parser, Debug)]
pub struct GetArgs {
    /// Type of resource to retrieve (e.g., nodes, pods)
    #[arg(value_enum)]
    resource: ResourceType,
}

/// Sends a GET request for the specified resource type and prints a table view.
pub async fn handle_get(config: &Config, args: &GetArgs) {
    let url = format!("{}/{}", &config.url, args.resource);
    let response = reqwest::get(&url).await;

    // Parse response and show in tabled
    match response {
        Ok(resp) if resp.status().is_success() => match args.resource {
            ResourceType::Nodes => match resp.json::<Vec<Node>>().await {
                Ok(data) => {
                    let mut table = Table::new(data);
                    table.with(Style::blank());
                    println!("{}", table);
                }
                Err(e) => eprintln!("Failed to parse nodes: {}", e),
            },
            ResourceType::Pods => match resp.json::<Vec<Pod>>().await {
                Ok(data) => {
                    let mut table = Table::new(data);
                    table.with(Style::blank());
                    println!("{}", table);
                }
                Err(e) => eprintln!("Failed to parse pods: {}", e),
            },
            ResourceType::Replicasets => match resp.json::<Vec<ReplicaSet>>().await {
                Ok(data) => {
                    let mut table = Table::new(data);
                    table.with(Style::blank());
                    println!("{}", table);
                }
                Err(e) => eprintln!("Failed to parse replicasets: {}", e),
            },
        },
        Ok(_) => {}
        Err(_) => {}
    }
}
