use std::env;

const R8S_SERVER_HOST: &str = "localhost";
const R8S_SERVER_PORT: u16 = 7620;

#[derive(Debug)]
pub struct Config {
    pub server: ServerConfig,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
}

pub fn load_config() -> Config {
    let address = env::var("R8S_SERVER_HOST").unwrap_or_else(|_| R8S_SERVER_HOST.to_string());

    let port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(R8S_SERVER_PORT);

    Config {
        server: ServerConfig { address, port },
    }
}
