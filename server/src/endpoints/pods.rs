use actix_web::{web, HttpResponse, Responder};
use crate::store::R8s;
use tracing::instrument;
use shared::api::{Spec, SpecObject};
use shared::api::{CreateResponse, PodQueryParams};


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("", web::get().to(get))
        .route("", web::post().to(create));
}


/// List, fetch and search pods
#[instrument(skip(state, query))]
async fn get(
    state: web::Data<R8s>,
    query: web::Query<PodQueryParams>) -> impl Responder {

    tracing::info!(?query.node_id, "Pod query");

    let pods = state.get_pods(query.node_id);
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&pods).unwrap())
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

            let response = CreateResponse {
                id,
                status: "Accepted".into(),
            };

            HttpResponse::Created().json(response)
        },
        _ => HttpResponse::MethodNotAllowed().finish()
    }
}
