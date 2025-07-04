use std::time::Duration;

use tokio::time;

use crate::state::State;

pub async fn run(state: State) -> Result<(), String> {
    let mut interval = time::interval(Duration::from_secs(state.config.sync_loop.into()));
    tracing::info!(sync=%state.config.sync_loop, "Starting sync loop");
    loop {
        interval.tick().await;
        tracing::warn!("Should send status, not implementd");
        if false {
            return Ok(());
        }
    }
}
