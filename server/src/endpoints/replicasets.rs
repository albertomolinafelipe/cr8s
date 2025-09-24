use crate::State;
use actix_web::{HttpResponse, Responder, web};
use shared::api::ReplicaSetManifest;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("", web::post().to(create));
}

async fn get(state: State) -> impl Responder {
    HttpResponse::NotImplemented().finish()
}

async fn create(state: State, payload: web::Json<ReplicaSetManifest>) -> impl Responder {
    // tracing::debug!("{0:?}", payload.spec);
    HttpResponse::NotImplemented().finish()
}
