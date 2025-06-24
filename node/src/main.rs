use tokio::sync::mpsc;
use tracing_subscriber;
use actix_web::web;
use uuid::Uuid;

mod api;
mod controller;
mod worker;
pub mod docker;
pub mod state;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let state = state::NodeState::new();
    let app_state = web::Data::new(state);
    tracing_subscriber::fmt::init();

    let (tx, rx) = mpsc::channel::<Uuid>(100);

    let server = api::run(app_state.clone()).await?;
    let server_handle = tokio::spawn(server);
    tokio::spawn(worker::run(app_state.clone(), rx));

    if let Err(_) = controller::run(app_state.clone(), tx).await {
        std::process::exit(1);
    }

    if let Err(_) = server_handle.await {
        std::process::exit(1);
    }
    Ok(())
}
