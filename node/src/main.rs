//! It sets up the application state, initializes logging, and runs core subsystems concurrently:
//! - API server
//! - Worker loop
//! - Sync logic
//! - Watcher loop
//!
//! Each subsystem communicates via a shared application state and message channels.

use tokio::sync::mpsc;
use tracing_subscriber::{self, EnvFilter};

use crate::{models::WorkRequest, state::NodeState};

mod api;
mod core;
mod docker;
pub mod models;
mod state;

#[tokio::main]
async fn main() -> Result<(), String> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let (tx, rx) = mpsc::channel::<WorkRequest>(100);
    let state = NodeState::new();

    tokio::try_join!(
        api::run(state.clone()),
        core::sync::run(state.clone()),
        core::worker::run(state.clone(), rx),
        core::watcher::run(state.clone(), tx),
    )?;

    Ok(())
}

#[cfg(test)]
mod test_setup {
    use std::sync::Once;
    static INIT: Once = Once::new();

    #[ctor::ctor]
    fn init_tracing() {
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_env_filter(format!("{}=trace", env!("CARGO_PKG_NAME")))
                .with_test_writer()
                .init();
        });
    }
}
