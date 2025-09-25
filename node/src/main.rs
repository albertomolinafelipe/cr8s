//! It sets up the application state, initializes logging, and runs core subsystems concurrently:
//! - API server
//! - Worker loop
//! - Sync logic
//! - Watcher loop
//!
//! Each subsystem communicates via a shared application state and message channels.

use cr8sagt::{
    api,
    core::{sync, watcher, worker},
    models::WorkRequest,
    state::NodeState,
};
use tokio::sync::mpsc;
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), String> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let (tx, rx) = mpsc::channel::<WorkRequest>(100);
    let state = NodeState::new();

    tokio::try_join!(
        api::run(state.clone()),
        sync::run(state.clone()),
        worker::run(state.clone(), rx),
        watcher::run(state.clone(), tx),
    )?;

    Ok(())
}
