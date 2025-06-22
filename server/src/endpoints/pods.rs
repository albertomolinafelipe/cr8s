use actix_web::{web::{self, Bytes}, HttpResponse, Responder};
use crate::store::R8s;
use shared::api::{PodManifest, CreateResponse, PodQueryParams};


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("", web::get().to(get))
        .route("", web::post().to(create));
}


/// List, fetch and search pods
async fn get(
    state: web::Data<R8s>,
    query: web::Query<PodQueryParams>,
) -> impl Responder {
    if query.watch.unwrap_or(false) {
        // Watch mode
        let mut rx = state.pod_tx.subscribe();
        let node_id = query.node_id.clone();

        let stream = async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if let Some(ref node_id) = node_id {
                    if event.pod.node_id != *node_id {
                        continue;
                    }
                }
                let json = serde_json::to_string(&event).unwrap();
                yield Ok::<_, actix_web::Error>(Bytes::from(json + "\n"));
            }
        };

        HttpResponse::Ok()
            .content_type("application/json")
            .streaming(stream)
    } else {
        // Normal list
        let pods = state.get_pods(query.node_id.clone());
        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string(&pods).unwrap())
    }
}


/// Add spec object to the system
async fn create(
    state: web::Data<R8s>,
    body: web::Json<PodManifest>,
) -> impl Responder {

    let spec_obj = body.into_inner();

    match state.add_pod(spec_obj.spec, spec_obj.metadata) {
        Ok(id) => {
            let response = CreateResponse {
                id,
                status: "Accepted".into(),
            };
            HttpResponse::Created().json(response)
        },
        Err(e) => HttpResponse::Conflict().body(e),
    }
        
}
