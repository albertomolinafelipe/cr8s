use actix_web::{dev::Server, web, App, HttpResponse, HttpServer, Responder};
use crate::config::Config;

pub async fn run(config: Config) -> std::io::Result<Server> {
    let server = HttpServer::new(move || {
        App::new()
            .route("/health", web::get().to(health_check))
            .route("/logs", web::get().to(logs))
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", config.port))?
    .workers(config.node_api_workers)
    .run();
    Ok(server)
}

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-node")
}

async fn health_check() -> impl Responder {
    HttpResponse::NotImplemented().finish()
}

async fn logs() -> impl Responder {
    HttpResponse::NotImplemented().finish()
}
