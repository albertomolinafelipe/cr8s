use actix_web::{dev::Server, web, App, HttpResponse, HttpServer, Responder};
use crate::state::State;

pub async fn run(state: State) -> std::io::Result<Server> {

    let port = state.config.port;
    let node_api_workers = state.config.node_api_workers;
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/pods", web::get().to(pods))
            .route("/id", web::get().to(id))
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", port))?
    .workers(node_api_workers)
    .run();
    Ok(server)
}

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-node")
}


async fn id(state: State) -> impl Responder {
    tracing::info!("Node id: {}", state.node_id());
    HttpResponse::Ok().body(state.node_id().to_string())
}

async fn pods(state: State) -> impl Responder {
    HttpResponse::Ok().json(&state.get_pods())
}
