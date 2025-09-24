use std::env;

const CR8S_SERVER_HOST: &str = "localhost";
const CR8S_SERVER_PORT: u16 = 7620;

#[derive(Debug)]
pub struct Config {
    pub url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let address = env::var("CR8S_SERVER_HOST").unwrap_or_else(|_| CR8S_SERVER_HOST.to_string());

        let port = env::var("CR8S_SERVER_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(CR8S_SERVER_PORT);

        Self {
            url: format!("http://{}:{}", address, port),
        }
    }
}
