use actix_web::{web, HttpResponse, Responder};
use crate::cluster::ClusterState;
use serde_json;

pub async fn handler(state: web::Data<ClusterState>) -> impl Responder {
    let nodes = state.get_nodes();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&nodes).unwrap())
}
