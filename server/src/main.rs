use actix_web::{App, HttpServer, web, HttpResponse, Responder};
use std::env;
use tracing_subscriber;

mod nodes;
mod pods;
mod store;

use store::R8s;

const R8S_SERVER_PORT: u16 = 7620;

#[actix_web::main]
async fn main() -> std::io::Result<()> {

    tracing_subscriber::fmt::init();
    let port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(R8S_SERVER_PORT);

    let state = web::Data::new(R8s::new());

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(nodes::config)
            .configure(pods::config)
            .route("/", web::get().to(root_handler))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

async fn root_handler() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-server")
}
