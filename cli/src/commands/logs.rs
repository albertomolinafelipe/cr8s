use crate::config::Config;
use clap::Parser;

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
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(body) => {
                    eprintln!("Response status: {}", status);
                    eprintln!("Response body:\n{}", body);
                }
                Err(err) => {
                    eprintln!("Failed to read response body: {}", err);
                }
            }
        }
        Err(err) => {
            eprintln!("Request error: {}", err);
        }
    }
}
