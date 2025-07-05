use crate::State;
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
async fn get(state: State, query: web::Query<PodQueryParams>) -> impl Responder {
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
    state: State,
    path_string: web::Path<String>,
    body: web::Json<PodStatusUpdate>,
) -> impl Responder {
    let mut status_update = body.into_inner();
    let pod_name = path_string.into_inner();

    // Check pod name exists
    let Some(pod_id) = state.pod_name_idx.get(&pod_name) else {
        return HttpResponse::NotFound().finish();
    };

    // Check node name and that pod is assigned to node
    if !state.node_names.contains(&status_update.node_name) {
        return HttpResponse::Forbidden().finish();
    }
    match state.pod_map.get(&status_update.node_name) {
        Some(set) if set.contains(&pod_id) => {}
        _ => return HttpResponse::Unauthorized().finish(),
    }

    // Update node heartbeat
    if let Err(error) = state.update_node_heartbeat(&status_update.node_name).await {
        tracing::warn!(error=%error, "Failed to update node heartbeat");
        // return error.to_http_response();
    }

    // Check body container names match spec
    match state
        .update_pod_status(
            pod_id.clone(),
            status_update.status.clone(),
            &mut status_update.container_statuses,
        )
        .await
    {
        Ok(_) => {
            tracing::trace!(
                pod=%pod_name,
                status=%status_update.status,
                "Pod status successfully updated"
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            tracing::warn!(
                error=%err,
                "Could not update pod status"
            );
            err.to_http_response()
        }
    }
}

/// Update pod
async fn update(
    state: State,
    path_string: web::Path<String>,
    body: web::Json<PodPatch>,
) -> impl Responder {
    let patch = body.into_inner();
    let pod_name = path_string.into_inner();
    match patch.pod_field {
        PodField::NodeName => match state.assign_pod(&pod_name, patch.value.clone()).await {
            Ok(_) => HttpResponse::NoContent().finish(),
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
async fn create(state: State, body: web::Json<PodManifest>) -> impl Responder {
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
