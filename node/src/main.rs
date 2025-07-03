use actix_web::web;
use tokio::sync::mpsc;
use tracing_subscriber::{self, EnvFilter};
use uuid::Uuid;

mod api;
mod controller;
pub mod docker;
pub mod state;
mod sync;
mod worker;

#[tokio::main]
async fn main() -> Result<(), String> {
    let state = web::Data::new(state::NodeState::new());

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("actix_server=warn,actix_web=warn,node=info"))
        .init();

    let (tx, rx) = mpsc::channel::<Uuid>(100);

    let controller_fut = controller::run(state.clone(), tx);
    let worker_fut = worker::run(state.clone(), rx);
    let sync_fut = sync::run(state.clone());
    let server_fut = api::run(state.clone());

    tokio::try_join!(server_fut, worker_fut, sync_fut, controller_fut,)?;

    Ok(())
}
