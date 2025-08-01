use crate::state::State;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use shared::api::LogsQuery;
use uuid::Uuid;

pub async fn run(state: State) -> Result<(), String> {
    tracing::info!("Starting api server");
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

async fn root() -> impl Responder {
    HttpResponse::Ok().body("Hello from r8s-node")
}

async fn pod_logs(
    state: State,
    path_string: web::Path<Uuid>,
    query: web::Query<LogsQuery>,
) -> impl Responder {
    let pod_id = path_string.into_inner();
    let follow = query.follow.unwrap_or(false);

    if follow {
        return HttpResponse::NotImplemented().finish();
    }

    let pod_runtime = match state.get_pod_runtime(&pod_id) {
        Some(p) => p,
        None => return HttpResponse::NotFound().body("Pod runtime not found in node cache"),
    };

    let container_id = match &query.container {
        Some(name) => {
            let Some(container) = pod_runtime.containers.get(name) else {
                return HttpResponse::NotFound().body("Specified container not found in runtime");
            };
            &container.id
        }
        None => {
            if pod_runtime.containers.len() != 1 {
                return HttpResponse::BadRequest()
                    .body("Container name is required for multi-container pods");
            }
            let container = pod_runtime.containers.values().next().unwrap();
            &container.id
        }
    };

    match state.docker_mgr.get_logs(container_id).await {
        Ok(logs) => HttpResponse::Ok().body(logs),
        Err(err) => {
            tracing::error!("Error getting pod logs: {}", err);
            HttpResponse::InternalServerError().finish()
        }
    }
}
