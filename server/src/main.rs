//! cr8s-server entrypoint.
//! Starts the Actix-web server and launches the scheduler and drift controller

use actix_web::{App, HttpServer};
use tracing_subscriber::{self, EnvFilter};

mod controllers;
mod endpoints;
mod state;

use endpoints::log::Logging;
use state::ApiServerState;

const DEFAULT_PORT: u16 = 7620;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    controllers::run().await;

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logging)
            .app_data(ApiServerState::new())
            .configure(endpoints::config)
    })
    .bind((
        "0.0.0.0",
        std::env::var("CR8S_SERVER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT),
    ))?;

    server.run().await
}
