use crate::config::Config;
use clap::Parser;
use futures_util::StreamExt;
use reqwest::StatusCode;
use tokio::io::{self, AsyncWriteExt};

#[derive(Parser, Debug)]
pub struct LogArgs {
    /// Name of the pod
    pub pod_name: String,
    /// Container name (optional, if the pod has multiple containers)
    #[arg(short = 'c', long = "container")]
    pub container: Option<String>,
    /// Follow the log stream
    #[arg(short = 'f', long = "follow")]
    pub follow: bool,
}

#[tokio::main]
pub async fn handle_logs(config: &Config, args: &LogArgs) {
    let mut url = format!("{}/pods/{}/logs", config.url, args.pod_name);
    let mut query = vec![];

    if let Some(container) = &args.container {
        query.push(format!("container={}", container));
    }

    if args.follow {
        query.push("follow=true".to_string());
    }

    if !query.is_empty() {
        url = format!("{}?{}", url, query.join("&"));
    }

    match reqwest::Client::new().get(&url).send().await {
        Ok(resp) => match resp.status() {
            StatusCode::OK => {
                if args.follow {
                    let mut stream = resp.bytes_stream();
                    let mut stdout = io::stdout();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                if let Err(e) = stdout.write_all(&bytes).await {
                                    eprintln!("Write error: {}", e);
                                    break;
                                }
                                let _ = stdout.flush().await;
                            }
                            Err(e) => {
                                eprintln!("Stream error: {}", e);
                                break;
                            }
                        }
                    }
                } else {
                    match resp.text().await {
                        Ok(body) => println!("{}", body),
                        Err(err) => eprintln!("Failed to read response body: {}", err),
                    }
                }
            }
            StatusCode::NOT_FOUND => {
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Not found".to_string());
                eprintln!("{}", body);
            }
            StatusCode::BAD_REQUEST => {
                eprintln!("Multicontainer pods require --container");
            }
            other => {
                let body = resp.text().await.unwrap_or_default();
                eprintln!("Unexpected status {}: {}", other, body);
            }
        },
        Err(err) => eprintln!("Request error: {}", err),
    }
}
