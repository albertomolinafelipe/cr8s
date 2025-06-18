use actix_web::{web, HttpResponse, Responder};
use crate::store::R8s;
use tracing::instrument;
use shared::api::PodQueryParams;

#[instrument(skip(_state, query))]
pub async fn handler(
    _state: web::Data<R8s>,
    query: web::Query<PodQueryParams>) -> impl Responder {

    tracing::info!(?query.node_name, "Pod query");

    HttpResponse::NotImplemented().finish()
}
