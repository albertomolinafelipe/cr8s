use tracing::instrument;
use reqwest::Client;
use shared::api::NodeRegisterReq;
use tokio::time::{sleep, Duration};
use crate::config::Config;

#[instrument(skip(config))]
pub async fn run(config: Config) -> Result<(), ()> {
    register(&config).await.map_err(|_| {
        tracing::error!("Failed to register after {} attempts", config.register_retries);
    })?;
    poll(&config).await.map_err(|_| {
        tracing::error!("Failed to poll");
    })?;

    Ok(())
}

#[instrument(skip(config))]
async fn poll(config: &Config) -> Result<(), ()> {
    let client = Client::new();
    
    for _ in 1..=5 {
        let node_info = NodeRegisterReq {
            port: config.port,
            name: config.name.clone(),
        };

        let response = client
            .get(format!("{}/pods?nodeName={}", config.server_url, config.name))
            .json(&node_info)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                tracing::warn!("Poll attemp failed: HTTP {}", resp.status());
            }
            Err(err) => {
                tracing::warn!("Poll attemp failed: {}", err);
            }
        }

        sleep(Duration::from_secs(5)).await;
    }

    Err(())
}

#[instrument]
async fn register(config: &Config) -> Result<(), ()> {
    let client = Client::new();

    for attempt in 1..=config.register_retries {
        let node_info = NodeRegisterReq {
            port: config.port,
            name: config.name.clone(),
        };

        let response = client
            .post(format!("{}/nodes", config.server_url))
            .json(&node_info)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                tracing::warn!("Register attempt {} failed: HTTP {}", attempt, resp.status());
            }
            Err(err) => {
                tracing::warn!("Register attempt {} failed: {}", attempt, err);
            }
        }

        sleep(Duration::from_secs(2)).await;
    }

    Err(())
}
