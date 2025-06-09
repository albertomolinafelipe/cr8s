use actix_web::{App, HttpServer, web};
use std::env;

mod nodes;
mod cluster;

use cluster::ClusterState;

const R8S_SERVER_PORT: u16 = 7620;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = env::var("R8S_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(R8S_SERVER_PORT);

    let state = web::Data::new(ClusterState::new());

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(nodes::config)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
