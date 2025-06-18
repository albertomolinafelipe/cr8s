use actix_web::{web, HttpRequest, HttpResponse, Responder};
use tracing::{Level, event, instrument};
use shared::{
    models::{NodeStatus, Node},
    api::NodeRegisterReq
};
use uuid::Uuid;

use crate::store::R8s;


#[instrument(skip(state, req, payload), fields(ip, port, name))]
pub async fn handler(
    req: HttpRequest,
    state: web::Data<R8s>,
    payload: web::Json<NodeRegisterReq>,
) -> impl Responder {

    let address = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let node = Node {
        id: Uuid::new_v4(),
        name: payload.name.clone(),
        api_url: format!("http://{}:{}", address, payload.port),
        status: NodeStatus::Ready,
        started_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now()
    };

    event!(
        Level::INFO,
        ip = %address,
        name = %node.name,
        "Node registered"
    );

    state.add_node(node);
    HttpResponse::Ok().body("Registered")
}
