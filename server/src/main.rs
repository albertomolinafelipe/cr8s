//! cr8s-server entrypoint.
//! Starts the Actix-web server and launches the scheduler and drift controller

use actix_web::{App, HttpServer};
use tracing_subscriber::{self, EnvFilter};

mod controllers;
mod endpoints;
mod state;

use state::ApiServerState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let state = ApiServerState::new().await;
    let port = std::env::var("CR8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7620);

    controllers::run(format!("http://localhost:{}", port));

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(endpoints::config)
            .wrap(endpoints::Logging)
    })
    .bind(("0.0.0.0", port))?;

    server.run().await
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
