use reqwest::Client;
use tokio::io::{AsyncBufReadExt, BufReader};
use shared::api::{CreateResponse, NodeRegisterReq, PodEvent};
use tokio::time::{sleep, Duration};
use tokio_util::io::StreamReader;
use futures_util::TryStreamExt;
use crate::{state::State, runtime::handler::handle_event};


pub async fn run(state: State) -> Result<(), ()> {
    register(state.clone()).await?;
    watch(state.clone()).await?;

    Ok(())
}


async fn watch(state: State) -> Result<(), ()> {
    let client = Client::new();

    let url = format!(
        "{}/pods?nodeId={}&watch=true",
        state.config.server_url, state.node_id()
    );

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let byte_stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

            let stream_reader = StreamReader::new(byte_stream);
            let mut lines = BufReader::new(stream_reader).lines();

            tracing::info!("Started watching pod assignments for {}", state.node_id());

            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<PodEvent>(&line) {
                    Ok(event) => handle_event(event).await,
                    Err(e) => {
                        tracing::warn!("Failed to deserialize line: {}\nError: {}", line, e);
                    }
                }
            }
            tracing::warn!("Watch stream ended.");
        }
        Ok(resp) => {
            tracing::error!("Watch request failed: HTTP {}", resp.status());
        }
        Err(err) => {
            tracing::error!("Watch request error: {}", err);
        }
    }

    Ok(())
}


async fn register(state: State) -> Result<(), ()> {
    let client = Client::new();

    for attempt in 1..=state.config.register_retries {
        let node_info = NodeRegisterReq {
            port: state.config.port,
            name: state.config.name.clone(),
        };

        let response = client
            .post(format!("{}/nodes", state.config.server_url))
            .json(&node_info)
            .send()
            .await;

        println!("URL: {}", state.config.server_url);

        match response {
            Ok(resp) if resp.status().is_success() => {
                let parsed = resp.json::<CreateResponse>().await.map_err(|e| {
                    tracing::warn!("Failed to parse register response: {}", e);
                })?;

                state.set_id(parsed.id);
                println!("r8s-node ready: {}", parsed.id);
                tracing::info!("Registered in the system: {}", parsed.id);

                return Ok(());
            },
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
