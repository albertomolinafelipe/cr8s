use clap::Parser;
use tabled::{Table, settings::Style};
use shared::Node;

use crate::config::Config;
use super::ResourceType;

#[derive(Parser, Debug)]
pub struct GetArgs {
    #[arg(value_enum)]
    resource: ResourceType,
}

#[tokio::main]
pub async fn handle(config: &Config, args: &GetArgs) {
    if args.resource != ResourceType::Nodes {
        println!("not implemented...");
        return;
    }

    let url = format!(
        "http://{}:{}/{}",
        &config.server.address, config.server.port, args.resource
    );

    let res = reqwest::get(&url).await;

    match res {
        Ok(response) => {
            match response.json::<Vec<Node>>().await {
                Ok(nodes) => {
                    let mut table = Table::new(nodes);
                    table.with(Style::blank());
                    println!("{}", table);
                }
                Err(e) => eprintln!("Failed to parse JSON: {}", e),
            }
        }
        Err(e) => eprintln!("Request failed: {}", e),
    }
}
