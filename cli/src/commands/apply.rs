use clap::Parser;
use shared::api::SpecObject;
use serde::de::Deserialize;
use tokio::fs;

use crate::config::Config;

#[derive(Parser, Debug)]
pub struct ApplyArgs {
    /// Path to the YAML file containing the deployment spec
    #[clap(short = 'f', long = "file")]
    pub file: String,
}


#[tokio::main]
pub async fn handle(config: &Config, args: &ApplyArgs) {
    let content = match fs::read_to_string(&args.file).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file '{}': {}", args.file, e);
            return;
        }
    };

    let docs: Vec<SpecObject> = match serde_yaml::Deserializer::from_str(&content)
        .map(|doc| serde_yaml::from_value(serde_yaml::Value::deserialize(doc).unwrap()))
        .collect::<Result<_, _>>()
        {
            Ok(pods) => pods,
            Err(e) => {
                eprintln!("Failed to parse YAML: {}", e);
                return;
            }
        };

    for object in docs {

        let url = format!("{}/{}s", config.url, object.spec);

        let client = reqwest::Client::new();
        let res = match client.post(&url)
            .json(&object)
            .send()
            .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("Failed to send request to {}: {}", url, e);
                    continue;
                }
            };

        println!("Response({}): {} {}", url, res.status(), res.text().await.unwrap_or_default());
    }

}
