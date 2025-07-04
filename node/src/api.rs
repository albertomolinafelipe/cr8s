use crate::state::State;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};

pub async fn run(state: State) -> Result<(), String> {
    tracing::info!("Starting api server");
    let port = state.config.port;
    let node_api_workers = state.config.node_api_workers;
    let _ = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/pods", web::get().to(pods))
            .route("/name", web::get().to(name))
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", port))
    .map_err(|e| e.to_string())?
    .workers(node_api_workers)
    .run();
    Ok(())
}

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-node")
}

async fn name(state: State) -> impl Responder {
    tracing::info!("Node id: {}", state.node_name());
    HttpResponse::Ok().body(state.node_name().to_string())
}

async fn pods(state: State) -> impl Responder {
    HttpResponse::Ok().json(&state.get_pod_names())
}
