use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use std::env;
use tracing_subscriber::{self, EnvFilter};

mod drift_controller;
mod endpoints;
mod scheduler;
mod store;

use store::state::{R8s, new_state};

const R8S_SERVER_PORT: u16 = 7620;
type State = web::Data<R8s>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("actix_server=warn,actix_web=warn,server=trace"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    let port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(R8S_SERVER_PORT);
    let state: State = new_state().await;

    // Start controller and scheduler
    if env::var("RUN_SCHEDULER")
        .map(|v| v != "false")
        .unwrap_or(true)
    {
        tokio::spawn(scheduler::run());
    }

    if env::var("RUN_DRIFT").map(|v| v != "false").unwrap_or(true) {
        tokio::spawn(drift_controller::run());
    }

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(endpoints::config)
            .route("/", web::get().to(root_handler))
    })
    .bind(("0.0.0.0", port))?;

    println!("r8s-server ready");
    server.run().await
}

async fn root_handler() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-server")
}
