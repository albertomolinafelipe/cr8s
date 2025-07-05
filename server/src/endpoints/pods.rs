use std::ops::Deref;

use crate::store::R8s;
use actix_web::{
    HttpResponse, Responder,
    web::{self, Bytes},
};
use shared::api::{
    CreateResponse, EventType, PodEvent, PodField, PodManifest, PodPatch, PodQueryParams,
    PodStatusUpdate,
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("/{pod_name}", web::patch().to(update))
        .route("/{pod_name}/status", web::post().to(status))
        .route("", web::post().to(create));
}

/// List, fetch and search pods
async fn get(state: web::Data<R8s>, query: web::Query<PodQueryParams>) -> impl Responder {
    tracing::debug!(
        watch=%query.watch.unwrap_or(false),
        node_name=%query.node_name.clone().unwrap_or("None".to_string()),
        "Get pod request");
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

/// Update pod status
async fn status(
    state: web::Data<R8s>,
    path_string: web::Path<String>,
    body: web::Json<PodStatusUpdate>,
) -> impl Responder {
    let status_update = body.into_inner();
    let pod_name = path_string.into_inner();

    // Check pod name and id exists
    match state.pod_name_idx.get(&pod_name) {
        Some(id) => {
            if id.deref() != &status_update.id {
                return HttpResponse::BadRequest().body("Pod id and pod name don't match");
            }
        }
        None => return HttpResponse::NotFound().finish(),
    }

    // Check node name and that pod is assigned to node
    if !state.node_names.contains(&status_update.node_name) {
        return HttpResponse::Forbidden().finish();
    }
    match state.pod_map.get(&status_update.node_name) {
        Some(set) if set.contains(&status_update.id) => {}
        _ => return HttpResponse::Unauthorized().finish(),
    }

    // Update node heartbeat
    if let Err(error) = state.update_node_heartbeat(&status_update.node_name).await {
        tracing::warn!(error=%error, "Failed to update node heartbeat");
        // return error.to_http_response();
    }
    tracing::trace!("Updated node heartbeat");

    // Check body container names match spec
    // Update status
    HttpResponse::NotImplemented().finish()
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
        PodField::NodeName => match state.assign_pod(&pod_name, patch.value.clone()).await {
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
        },
        PodField::PodStatus => HttpResponse::NoContent().finish(),
    }
}

/// Add spec object to the system
async fn create(state: web::Data<R8s>, body: web::Json<PodManifest>) -> impl Responder {
    let spec_obj = body.into_inner();
    let pod_name = spec_obj.metadata.name.clone();
    tracing::debug!(name=%pod_name, "Received pod manifest");

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
