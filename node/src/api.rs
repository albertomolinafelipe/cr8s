//! # Node API Server
//! This module defines the HTTP API exposed by the node agent

use crate::state::State;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use bytes::Bytes;
use futures_util::StreamExt;
use shared::api::LogsQueryParams;
use uuid::Uuid;

/// Routes:
/// - `GET /pods/{pod_id}/logs`: Retrieves logs for a specific pod container.
pub async fn run(state: State) -> Result<(), String> {
    let port = state.config.port;
    let node_api_workers = state.config.node_api_workers;

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/pods/{pod_id}/logs", web::get().to(pod_logs))
            .route("/", web::get().to(root))
    })
    .bind(("0.0.0.0", port))
    .map_err(|e| e.to_string())?
    .workers(node_api_workers)
    .run()
    .await
    .map_err(|e| e.to_string())
}

/// Root endpoint handler.
async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-node")
}

/// Retrieves logs for a specific pod container.
///
/// Supports both static logs and streaming logs using the `follow` query param.
/// If multiple containers exist, `container` query param must be specified.
///
/// # Query Parameters
/// - `follow`: If true, stream logs.
/// - `container`: (optional) container name in a multi-container pod.
///
/// # Path Parameters
/// - `pod_id`: UUID of the pod.
///
/// # Returns
/// - `200 OK` with logs or log stream.
/// - `404 Not Found` if the pod or container is not present.
/// - `400 Bad Request` if container name is required but not provided.
async fn pod_logs(
    state: State,
    path_string: web::Path<Uuid>,
    query: web::Query<LogsQueryParams>,
) -> impl Responder {
    let pod_id = path_string.into_inner();
    let follow = query.follow.unwrap_or(false);

    // get pod runtime info
    let pod_runtime = match state.get_pod_runtime(&pod_id) {
        Some(p) => p,
        None => return HttpResponse::NotFound().body("Pod runtime not found in node cache"),
    };

    // container id given by docker api
    let container_id = match &query.container {
        Some(name) => {
            let Some(container) = pod_runtime.containers.get(name) else {
                return HttpResponse::NotFound().body("Specified container not found in runtime");
            };
            &container.id
        }
        // when no container name was given
        // - multicontainer pods will get 400
        // - otherwise get logs for only container
        None => {
            if pod_runtime.containers.len() != 1 {
                return HttpResponse::BadRequest()
                    .body("Container name is required for multi-container pods");
            }
            let container = pod_runtime.containers.values().next().unwrap();
            &container.id
        }
    };

    if follow {
        match state.docker_mgr.stream_logs(container_id).await {
            Ok(stream) => {
                let byte_stream = stream.map(|res| match res {
                    Ok(bytes) => Ok::<Bytes, actix_web::Error>(bytes),
                    Err(err) => {
                        tracing::error!("Stream error: {}", err);
                        Err(actix_web::error::ErrorInternalServerError(
                            "streaming error",
                        ))
                    }
                });

                HttpResponse::Ok()
                    .content_type("text/plain")
                    .streaming(byte_stream)
            }

            Err(err) => {
                tracing::error!("Error streaming logs: {}", err);
                HttpResponse::InternalServerError().body("Error streaming logs")
            }
        }
    } else {
        match state.docker_mgr.get_logs(container_id).await {
            Ok(logs) => HttpResponse::Ok().body(logs),
            Err(err) => {
                tracing::error!("Error getting pod logs: {}", err);
                HttpResponse::InternalServerError().body("Error fetching logs")
            }
        }
    }
}
