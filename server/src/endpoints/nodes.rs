use crate::State;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    web::{self, Bytes},
};
use serde::Deserialize;
use shared::{
    api::{EventType, NodeEvent, NodeRegisterReq},
    models::{Node, NodeStatus},
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("", web::post().to(register));
}

#[derive(Deserialize)]
pub struct NodeQuery {
    watch: Option<bool>,
}

/// List, fetch and search pods
async fn get(state: State, query: web::Query<NodeQuery>) -> impl Responder {
    if query.watch.unwrap_or(false) {
        // Watch mode
        let mut rx = state.node_tx.subscribe();
        let nodes = state.get_nodes().await;
        let stream = async_stream::stream! {
            for n in nodes {
                let event = NodeEvent {
                    node: n,
                    event_type: EventType::Added
                };
                let json = serde_json::to_string(&event).unwrap();
                yield Ok::<_, actix_web::Error>(Bytes::from(json + "\n"));
            }
            while let Ok(event) = rx.recv().await {
                let json = serde_json::to_string(&event).unwrap();
                yield Ok::<_, actix_web::Error>(Bytes::from(json + "\n"));
            }
        };

        HttpResponse::Ok()
            .content_type("application/json")
            .streaming(stream)
    } else {
        // Normal list
        let nodes = state.get_nodes().await;
        tracing::info!(num_nodes = nodes.len(), "Retrieved cluster nodes");
        HttpResponse::Ok().json(&nodes)
    }
}

/// Nodes register to the service
async fn register(
    req: HttpRequest,
    state: State,
    payload: web::Json<NodeRegisterReq>,
) -> impl Responder {
    let address = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let node = Node {
        name: payload.name.clone(),
        addr: format!("{}:{}", address, payload.port),
        status: NodeStatus::Ready,
        started_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
    };

    match state.add_node(&node).await {
        Ok(()) => {
            tracing::info!(
                ip=%address,
                name=%node.name,
                "Node registered"
            );
            HttpResponse::Created().finish()
        }
        Err(err) => {
            tracing::warn!(
                error=%err,
                "Could not register node"
            );
            err.to_http_response()
        }
    }
}
