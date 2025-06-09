use std::env;
use reqwest::Client;
use serde::Serialize;

const R8S_SERVER_HOST: &str = "localhost";
const R8S_SERVER_PORT: u16 = 7620;

#[derive(Debug)]
pub struct Config {
    pub server_address: String,
    pub server_port: u16,
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct NodeInfo {
    pub port: u16,
}

#[tokio::main]
async fn main() {
    let config = load_config();

    let url = format!(
        "http://{}:{}/nodes/register",
        config.server_address,
        config.server_port
    );

    let client = Client::new();
    let node_info = NodeInfo {
        port: config.port,
    };

    match client.post(url)
        .json(&node_info)
        .send()
        .await
    {
        Ok(_) => {},
        Err(e) => eprintln!("error sending register: {}", e),
    };
}

fn load_config() -> Config {
    let server_address = env::var("R8S_SERVER_HOST")
        .unwrap_or_else(|_| R8S_SERVER_HOST.to_string());

    let server_port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(R8S_SERVER_PORT);

    let port = env::var("NODE_PORT")
        .expect("NODE_PORT environment variable is required")
        .parse()
        .expect("NODE_PORT must be a valid number");

    Config {
        server_address,
        server_port,
        port,
    }
}
