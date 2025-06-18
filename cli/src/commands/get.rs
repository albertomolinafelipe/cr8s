use clap::Parser;
use tabled::{Table, settings::Style};
use shared::models::Node;

use crate::config::Config;
use super::ResourceType;

#[derive(Parser, Debug)]
pub struct GetArgs {
    #[arg(value_enum)]
    resource: ResourceType,
}

#[tokio::main]
pub async fn handle(config: &Config, args: &GetArgs) {

    let url = format!(
        "http://{}/{}",
        &config.url, args.resource
    );

    let response = reqwest::get(&url).await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Vec<Node>>().await {
                Ok(nodes) => {
                    let mut table = Table::new(nodes);
                    table.with(Style::blank());
                    println!("{}", table);
                }
                Err(e) => eprintln!("Failed to parse JSON: {}", e),
            }
        }
        Ok(resp) => eprintln!("Failed: {:#?}", resp.error_for_status()),
        Err(e) => eprintln!("Request failed: {}", e),
    }
}
