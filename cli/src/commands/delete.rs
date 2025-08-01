use clap::Parser;
use reqwest::StatusCode;

use crate::{commands::ResourceKind, config::Config};

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    /// Name of pod to delete
    #[arg(value_enum)]
    resource: ResourceKind,
    identifier: String,
}

#[tokio::main]
pub async fn handle(config: &Config, args: &DeleteArgs) {
    match args.resource {
        ResourceKind::Pod => {
            let url = format!("{}/{}s/{}", &config.url, args.resource, args.identifier);
            match reqwest::Client::new().delete(&url).send().await {
                Ok(resp) if resp.status() == StatusCode::NO_CONTENT => {}
                Ok(_) => eprintln!("Error deleting resource"),
                Err(_) => eprintln!("Error sending request"),
            }
        }
        ResourceKind::Deployment => println!("not implemented"),
    }
}
