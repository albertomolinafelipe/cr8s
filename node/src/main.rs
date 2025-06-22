use tracing_subscriber;
use actix_web::web;

mod node_api;
mod runtime;
pub mod state;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let state = state::NodeState::new();
    let app_state = web::Data::new(state);
    tracing_subscriber::fmt::init();

    let server = node_api::run(app_state.clone()).await?;
    let server_handle = tokio::spawn(server);

    if let Err(_) = runtime::run(app_state.clone()).await {
        std::process::exit(1);
    }

    if let Err(_) = server_handle.await {
        std::process::exit(1);
    }
    Ok(())
}
