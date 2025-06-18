use tracing_subscriber;

mod node_api;
mod runtime;
pub mod config;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = config::load_config();
    tracing_subscriber::fmt::init();

    let server = node_api::run(config.clone()).await?;
    let server_handle = tokio::spawn(server);

    if let Err(_) = runtime::run(config).await {
        std::process::exit(1);
    }

    if let Err(_) = server_handle.await {
        std::process::exit(1);
    }

    Ok(())
}
