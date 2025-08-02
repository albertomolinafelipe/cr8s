//! CLI `logs` command: fetches logs of a pod's container,
//! optionally following the log stream live.

use crate::config::Config;
use clap::Parser;
use futures_util::StreamExt;
use reqwest::StatusCode;
use tokio::io::{self, AsyncWriteExt};

/// CLI arguments for the `logs` command.
#[derive(Parser, Debug)]
pub struct LogArgs {
    /// Name of the pod
    pub pod_name: String,
    /// Container name (optional, if the pod has multiple containers)
    #[arg(short = 'c', long = "container")]
    pub container: Option<String>,
    /// Follow the log stream live
    #[arg(short = 'f', long = "follow")]
    pub follow: bool,
}

/// Handles fetching and displaying pod logs.
/// Supports streaming logs when `--follow` is enabled.
#[tokio::main]
pub async fn handle_logs(config: &Config, args: &LogArgs) {
    let mut url = format!("{}/pods/{}/logs", config.url, args.pod_name);
    let mut query = vec![];

    // Build url with cli flags
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
                    // Stream logs in chunks, writing to stdout
                    let mut stream = resp.bytes_stream();
                    let mut stdout = io::stdout();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                if let Err(e) = stdout.write_all(&bytes).await {
                                    break;
                                }
                                let _ = stdout.flush().await;
                            }
                            Err(_) => break,
                        }
                    }
                } else {
                    // Print entire log at once
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
            StatusCode::BAD_REQUEST => eprintln!("Multicontainer pods require --container"),
            _ => {}
        },
        Err(_) => {}
    }
}
