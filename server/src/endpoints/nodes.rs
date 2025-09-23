//! Node Controller
//!
//! This module defines HTTP handlers for managing cluster nodes. It provides
//! endpoints for node registration and for listing or watching registered nodes.
//!
//! ## Routes
//! - `GET  /nodes`  — List or watch all registered nodes
//! - `POST /nodes`  — Register a new node with the control plane

use crate::State;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    web::{self, Bytes},
};
use serde::Deserialize;
use shared::{
    api::{EventType, NodeEvent, NodeRegisterReq},
    models::node::{Node, NodeStatus},
};
use uuid::Uuid;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("", web::post().to(register));
}

#[derive(Deserialize)]
pub struct NodeQuery {
    watch: Option<bool>,
}

/// List or watch nodes
///
/// # Arguments
/// - `query`: Query parameters:
///    - `watch` (bool, optional): If true, opens a watch stream of node events.
///
/// # Returns
/// - 200 list of nodes or stream of node events
async fn get(state: State, query: web::Query<NodeQuery>) -> impl Responder {
    tracing::trace!(
        watch=%query.watch.unwrap_or(false),
        "Get node request");
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
        HttpResponse::Ok().json(&nodes)
    }
}

/// Register a new node with the control plane.
///
/// # Arguments
/// - `payload`: Node register JSON
///
/// # Returns
/// - 201: Node successfully registered.
/// - 400: Emtpy node name
/// - 409: Duplicate name or address
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
        id: Uuid::new_v4(),
        name: payload.name.clone(),
        addr: format!("{}:{}", address, payload.port),
        status: NodeStatus::Ready,
        started_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
    };

    // validate node name and check for name and addr duplicates
    if node.name.is_empty() {
        return HttpResponse::BadRequest().body("Node name is empty");
    };
    if state.cache.node_addr_exists(&node.addr) || state.cache.node_name_exists(&node.name) {
        return HttpResponse::Conflict().body("Duplicate node name or address");
    };

    // Store node
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

#[cfg(test)]
mod tests {
    //!  GET
    //!  - test_get_nodes  
    //!  - test_get_nodes_empty
    //!  - test_get_nodes_watch
    //!         nodes added before and after watch call
    //!
    //!  REGISTER
    //!  - test_register_node
    //!  - test_register_node_empty_name
    //!  - test_register_node_repeat_name
    //!  - test_register_node_repeat_addr

    use crate::endpoints::helpers::collect_stream_events;
    use crate::store::{new_state_with_store, test_store::TestStore};

    use super::*;
    use actix_web::body::BoxBody;
    use actix_web::dev::Service;
    use actix_web::{
        App,
        http::StatusCode,
        test::{self, TestRequest, call_service, init_service, read_body_json},
    };

    async fn node_service(
        state: &State,
    ) -> impl Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<BoxBody>,
        Error = actix_web::Error,
    > {
        init_service(
            App::new()
                .app_data(state.clone())
                .route("/nodes", web::get().to(get))
                .route("/nodes", web::post().to(register)),
        )
        .await
    }

    #[actix_web::test]
    async fn test_get_nodes_empty() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let app = node_service(&state).await;

        let req = TestRequest::get().uri("/nodes").to_request();
        let res = call_service(&app, req).await;

        assert!(res.status().is_success());
        let nodes: Vec<Node> = read_body_json(res).await;
        assert!(nodes.is_empty(), "Node list should be empty");
    }

    #[actix_web::test]
    async fn test_get_nodes() {
        let test_store = TestStore::new();
        let node = Node::default();
        test_store.nodes.insert(node.name.clone(), node);
        let state = new_state_with_store(Box::new(test_store)).await;

        let app = node_service(&state).await;
        let req = TestRequest::get().uri("/nodes").to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
        let nodes: Vec<Node> = read_body_json(res).await;
        assert_eq!(nodes.len(), 1, "Node list should have one item");
    }

    #[actix_web::test]
    async fn test_get_nodes_watch() {
        let test_store = TestStore::new();
        let n1 = Node {
            name: "n1".to_string(),
            ..Default::default()
        };
        test_store.nodes.insert(n1.name.clone(), n1.clone());
        let state = new_state_with_store(Box::new(test_store)).await;

        let app = node_service(&state).await;

        let req = test::TestRequest::get()
            .uri("/nodes?watch=true")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let n2 = Node {
            name: "n1".to_string(),
            ..Default::default()
        };
        assert!(state.add_node(&n2).await.is_ok());
        assert!(resp.status().is_success());

        let mut events: Vec<NodeEvent> = Vec::new();
        collect_stream_events(resp, &mut events, 2).await;

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].node.name, n1.name);
        assert_eq!(events[1].node.name, n2.name);
    }

    #[actix_web::test]
    async fn test_register_node() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;

        let app = node_service(&state).await;
        let payload = NodeRegisterReq {
            port: 1000,
            name: "n1".to_string(),
        };
        let req = TestRequest::post()
            .uri("/nodes")
            .set_json(&payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[actix_web::test]
    async fn test_register_node_empty_name() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let app = node_service(&state).await;

        let payload = NodeRegisterReq {
            port: 1000,
            name: "".to_string(),
        };
        let req = TestRequest::post()
            .uri("/nodes")
            .set_json(&payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_register_node_repeat_name() {
        let n1 = Node {
            name: "n1".to_string(),
            ..Default::default()
        };
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        assert!(state.add_node(&n1).await.is_ok());

        let app = node_service(&state).await;
        let payload = NodeRegisterReq {
            port: 1000,
            name: "n1".to_string(),
        };
        let req = TestRequest::post()
            .uri("/nodes")
            .set_json(&payload)
            .to_request();
        let res = call_service(&app, req).await;

        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_register_node_repeat_addr() {
        let n1 = Node {
            addr: "unknown:1000".to_string(),
            ..Default::default()
        };
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        assert!(state.add_node(&n1).await.is_ok());

        let app = node_service(&state).await;
        let payload = NodeRegisterReq {
            port: 1000,
            name: "n2".to_string(),
        };
        let req = TestRequest::post()
            .uri("/nodes")
            .set_json(&payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CONFLICT,);
    }
}
