use actix_web::{web, HttpResponse, Responder};
use crate::store::R8s;
use tracing::instrument;
use serde_json::json;
use shared::models::{Spec, SpecObject};
use shared::api::PodQueryParams;


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("", web::get().to(get))
        .route("", web::post().to(create));
}


/// List, fetch and search pods
#[instrument(skip(_state, query))]
async fn get(
    _state: web::Data<R8s>,
    query: web::Query<PodQueryParams>) -> impl Responder {

    tracing::info!(?query.node_name, "Pod query");

    HttpResponse::NotImplemented().finish()
}


/// Add spec object to the system
#[instrument(skip(state))]
async fn create(
    state: web::Data<R8s>,
    body: web::Json<SpecObject>,
) -> impl Responder {

    let spec_obj = body.into_inner();
    match spec_obj.spec {
        Spec::Pod(spec) => {
            let id = state.add_pod(spec, spec_obj.metadata);
            
            HttpResponse::Created().json(json!({
                "uid": id,
                "status": "Accepted"
            }))
        },
        _ => HttpResponse::MethodNotAllowed().finish()
    }
}
