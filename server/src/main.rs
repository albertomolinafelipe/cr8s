//! r8s-server entrypoint.
//! Starts the Actix-web server and launches the scheduler and drift controller

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use std::env;
use tracing_subscriber::{self, EnvFilter};

mod drift_controller;
mod endpoints;
mod scheduler;
mod store;

use store::state::{R8s, new_state};

const DEFAULT_PORT: u16 = 7620;
type State = web::Data<R8s>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn,server=trace"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let config = Config::from_env();
    let state: State = new_state().await;

    // Start background scheduler and drift controller
    if config.run_scheduler {
        tokio::spawn(scheduler::run());
    }
    if config.run_drift {
        tokio::spawn(drift_controller::run());
    }

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(endpoints::config)
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", config.port))?;

    server.run().await
}

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-server")
}

struct Config {
    port: u16,
    run_scheduler: bool,
    run_drift: bool,
}

impl Config {
    fn from_env() -> Self {
        Self {
            port: env::var("R8S_SERVER_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PORT),

            run_scheduler: env::var("RUN_SCHEDULER")
                .map(|v| v != "false")
                .unwrap_or(true),

            run_drift: env::var("RUN_DRIFT").map(|v| v != "false").unwrap_or(true),
        }
    }
}
