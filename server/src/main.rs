//! cr8s-server entrypoint.
//! Starts the Actix-web server and launches the scheduler and drift controller

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use std::env;
use tracing_subscriber::{self, EnvFilter};

mod controllers;
mod endpoints;
mod scheduler;
mod store;

use endpoints::log::Logging;
use store::{Cr8s, new_state};

const DEFAULT_PORT: u16 = 7620;
type State = web::Data<Cr8s>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let config = Config::from_env();
    let state: State = new_state().await;

    // Start background scheduler and controllers
    tokio::spawn(scheduler::run());
    tokio::spawn(controllers::garbage_collector::run());
    tokio::spawn(controllers::replicaset::run());

    // Start apiserver
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logging)
            .app_data(state.clone())
            .configure(endpoints::config)
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", config.port))?;

    server.run().await
}

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from cr8s-server")
}

// ------------

struct Config {
    port: u16,
}

impl Config {
    fn from_env() -> Self {
        Self {
            port: env::var("CR8S_SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PORT),
        }
    }
}
