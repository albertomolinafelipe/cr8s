//! It sets up the application state, initializes logging, and runs core subsystems concurrently:
//! - API server
//! - Worker loop
//! - Sync logic
//! - Controller loop
//!
//! Each subsystem communicates via a shared application state and message channels.

use r8sagt::{
    api,
    core::{controller, sync, worker},
    models::WorkRequest,
    state::new_state,
};
use tokio::sync::mpsc;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), String> {
    let state = new_state().await;

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn,node=trace"));

    let node_name = state.config.name.clone();
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    let _ = tracing::info_span!("", node = %node_name).enter();

    let (tx, rx) = mpsc::channel::<WorkRequest>(100);

    let controller_fut = controller::run(state.clone(), tx);
    let worker_fut = worker::run(state.clone(), rx);
    let sync_fut = sync::run(state.clone());
    let api_fut = api::run(state.clone());

    tokio::try_join!(api_fut, worker_fut, sync_fut, controller_fut)?;

    Ok(())
}
