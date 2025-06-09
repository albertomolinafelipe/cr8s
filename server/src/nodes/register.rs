use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use shared::{Node, NodeStatus};

use crate::cluster::ClusterState;


#[derive(Deserialize)]
pub struct RegisterRequest {
    port: u16,
}

pub async fn handler(
    req: HttpRequest,
    state: web::Data<ClusterState>,
    payload: web::Json<RegisterRequest>,
) -> impl Responder {
    let address = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("Adding node: {}, {}", address, payload.port);
    let node = Node {
        address,
        port: payload.port.clone(),
        status: NodeStatus::Ready,
        name: "worker-node".to_string(),
        started_at: chrono::Utc::now()
    };
    state.add_node(node);
    HttpResponse::Ok().body("Registered")
}
