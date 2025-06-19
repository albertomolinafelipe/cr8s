use std::env;

use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub port: u16,
    pub name: String,
    pub register_retries: u16,
    pub node_api_workers: usize,
    pub node_id: Uuid
}


pub fn load_config() -> Config {
    let server_address = env::var("R8S_SERVER_HOST")
        .unwrap_or_else(|_| "localhost".to_string());
    
    let server_port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(7620);

    let port = env::var("NODE_PORT")
        .expect("NODE_PORT environment variable is required")
        .parse()
        .expect("NODE_PORT must be a valid number");
    
    let name = env::var("NODE_NAME")
        .unwrap_or_else(|_| format!("worker-node-{}", port));

    let register_retries = env::var("NODE_REGISTER_RETRIES")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3);
    
    let node_api_workers = env::var("NODE_API_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2);


    Config {
        server_url: format!("http://{}:{}", server_address, server_port),
        port,
        name,
        register_retries,
        node_api_workers,
        node_id: Uuid::nil()
    }
}
