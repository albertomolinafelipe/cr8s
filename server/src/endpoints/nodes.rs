use actix_web::{web, HttpRequest, HttpResponse, Responder};
use crate::store::R8s;
use serde_json::json;
use shared::{
    models::{NodeStatus, Node},
    api::NodeRegisterReq
};
use uuid::Uuid;
use tracing::instrument;


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("", web::post().to(register))
        .route("", web::get().to(get));
}


/// Get the list of nodes registered in the system
#[instrument(skip(state))]
async fn get(state: web::Data<R8s>) -> impl Responder {
    let nodes = state.get_nodes();
    tracing::info!(num_nodes = nodes.len(), "Retrieved cluster nodes");
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&nodes).unwrap())
}


/// Nodes register to the service
#[instrument(skip(state, req, payload), fields(ip, port, name))]
async fn register(
    req: HttpRequest,
    state: web::Data<R8s>,
    payload: web::Json<NodeRegisterReq>,
) -> impl Responder {

    let address = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let id = Uuid::new_v4();
    let node = Node {
        id,
        name: payload.name.clone(),
        api_url: format!("http://{}:{}", address, payload.port),
        status: NodeStatus::Ready,
        started_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now()
    };

    tracing::info!(
        ip = %address,
        name = %node.name,
        "Node registered"
    );

    state.add_node(node);
    HttpResponse::Created().json(json!({
        "uid": id,
        "status": "Accepted"
    }))
}
