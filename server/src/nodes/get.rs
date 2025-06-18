use actix_web::{web, HttpResponse, Responder};
use crate::store::R8s;
use serde_json;
use tracing::instrument;

#[instrument(skip(state))]
pub async fn handler(state: web::Data<R8s>) -> impl Responder {
    let nodes = state.get_nodes();
    tracing::info!(num_nodes = nodes.len(), "Retrieved cluster nodes");
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&nodes).unwrap())
}
