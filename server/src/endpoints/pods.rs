use actix_web::{web::{self, Bytes}, HttpResponse, Responder};
use crate::store::R8s;
use shared::api::{CreateResponse, EventType, PodEvent, PodField, PodManifest, PodPatch, PodQueryParams};


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("", web::get().to(get))
        .route("/{pod_name}", web::patch().to(update))
        .route("", web::post().to(create));
}


/// List, fetch and search pods
async fn get(
    state: web::Data<R8s>,
    query: web::Query<PodQueryParams>,
) -> impl Responder {
    if query.watch.unwrap_or(false) {
        // Watch mode
        let node_name = query.node_name.clone();
        let pods = state.get_pods(node_name.clone()).await;
        let stream = async_stream::stream! {
            // List all pods
            for p in &pods {
                let event = PodEvent {
                    pod: p.clone(),
                    event_type: EventType::Added,
                };
                if let Some(name) = node_name.as_deref() {
                    if event.pod.node_name != name {
                        continue;
                    }
                }
                let json = serde_json::to_string(&event).unwrap();
                yield Ok::<_, actix_web::Error>(Bytes::from(json + "\n"));
            }
            // Wacth new events
            let mut rx = state.pod_tx.subscribe();
            while let Ok(event) = rx.recv().await {
                if let Some(name) = node_name.as_deref() {
                    if event.pod.node_name != name {
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
        let pods = state.get_pods(query.node_name.clone()).await;
        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string(&pods).unwrap())
    }
}


/// Update pod
async fn update(
    state: web::Data<R8s>,
    path_string: web::Path<String>,
    body: web::Json<PodPatch>,
) -> impl Responder {
    let patch = body.into_inner();
    let pod_name = path_string.into_inner();
    match patch.pod_field {
        PodField::NodeName => {
            match state.assign_pod(&pod_name, patch.value.clone()).await {
                Ok(_) => {
                    tracing::info!(
                        pod=%pod_name,
                        node=%patch.value,
                        "Pod successfully assigned to node"
                    );
                    HttpResponse::NoContent().finish()
                }
                Err(err) => {
                    tracing::warn!(
                        error=%err,
                        "Could not schedule pod"
                    );
                    err.to_http_response()
                }
            }
        },
        PodField::PodStatus => HttpResponse::NoContent().finish()
    }
}


/// Add spec object to the system
async fn create(
    state: web::Data<R8s>,
    body: web::Json<PodManifest>,
) -> impl Responder {

    let spec_obj = body.into_inner();
    let pod_name = spec_obj.metadata.name.clone();
    tracing::info!(name=%pod_name, "Received pod manifest");

    match state.add_pod(spec_obj.spec, spec_obj.metadata).await {
        Ok(id) => {
            tracing::info!(
                name=%pod_name,
                "Pod created"
            );
            let response = CreateResponse {
                id,
                status: "Accepted".into(),
            };
            HttpResponse::Created().json(response)
        }
        Err(err) => {
            tracing::warn!(
                error=%err,
                "Could not create pod"
            );
            err.to_http_response()
        }
    }
}
