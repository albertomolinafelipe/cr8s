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
        .route("/{pod_name}/status", web::patch().to(status))
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
    let Some(pod_id) = state.cache.get_pod_id(&pod_name) else {
        return HttpResponse::NotFound().finish();
    };

    // Check node name and that pod is assigned to node
    if !state.cache.node_name_exists(&status_update.node_name) {
        return HttpResponse::Forbidden().finish();
    }
    match state.cache.get_pod_ids(&status_update.node_name) {
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
        PodField::Spec => HttpResponse::NotImplemented().finish(),
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

#[cfg(test)]
mod tests {
    use crate::endpoints::helpers::collect_stream_events;
    use crate::store::{state::new_state_with_store, test_store::TestStore};

    use super::*;
    use actix_web::body::BoxBody;
    use actix_web::dev::Service;
    use actix_web::{
        App,
        http::StatusCode,
        test::{self, TestRequest, call_service, init_service, read_body_json},
    };
    use shared::models::{ContainerSpec, Node, PodObject, PodSpec, PodStatus, UserMetadata};

    async fn pod_service(
        state: &State,
    ) -> impl Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<BoxBody>,
        Error = actix_web::Error,
    > {
        init_service(
            App::new()
                .app_data(state.clone())
                .route("/pods", web::get().to(get))
                .route("/pods/{pod_name}", web::patch().to(update))
                .route("/pods/{pod_name}/status", web::patch().to(status))
                .route("/pods", web::post().to(create)),
        )
        .await
    }

    async fn add_assigned_pod(state: &State) -> (String, String) {
        let n = Node::default();
        assert!(state.add_node(&n).await.is_ok());

        let pod_name = add_pod(state).await;
        assert!(state.assign_pod(&pod_name, n.name.clone()).await.is_ok());
        return (n.name.clone(), pod_name.clone());
    }

    async fn add_pod(state: &State) -> String {
        let spec = PodSpec::default();
        let metadata = UserMetadata::default();
        assert!(state.add_pod(spec, metadata.clone()).await.is_ok());
        return metadata.name;
    }

    ///  
    ///  GET
    ///  - test_get_pods_query
    ///  - test_get_pods_watch
    ///         pods added before and after watch call, assigned and unassigned
    ///
    ///  STATUS
    ///  - test_update_pod_status
    ///  - test_update_pod_status_pod_name_not_found
    ///  - test_update_pod_status_node_not_found
    ///  - test_update_pod_status_not_assigned_to_caller
    ///
    ///
    ///  PATCH POD
    ///  - test_assign_pod
    ///  - test_assign_pod_invalid_node_name
    ///  - test_assign_pod_not_found
    ///  - test_assign_pod_already_assigned
    ///  - test_update_pod_spec
    ///
    ///  CREATE
    ///  - test_create_pod
    ///  - test_create_pod_repeat_name
    ///  - test_create_pod_repeat_container_names
    ///

    #[actix_web::test]
    async fn test_get_pods_query() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let _ = add_pod(&state).await;

        let app = pod_service(&state).await;

        let req = TestRequest::get().uri("/pods").to_request();
        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let pods: Vec<PodObject> = read_body_json(resp).await;
        assert_eq!(pods.len(), 1, "There should be a single pod");
    }

    #[actix_web::test]
    async fn test_get_pods_watch() {
        // Add initial assigned pod
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let (node_name, pod_name_1) = add_assigned_pod(&state).await;

        let app = pod_service(&state).await;

        let req = test::TestRequest::get()
            .uri(&format!("/pods?watch=true&nodeName={}", node_name))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Add second unassigned pod
        let pod_name_2 = add_pod(&state).await;

        // Check events for assigned pod
        let mut events: Vec<PodEvent> = Vec::new();
        collect_stream_events(resp, &mut events, 1).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pod.metadata.user.name, pod_name_1);
        assert_eq!(events[0].pod.node_name, node_name);

        let req = test::TestRequest::get()
            .uri("/pods?watch=true&nodeName=")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Check events for unassigned
        let mut events: Vec<PodEvent> = Vec::new();
        collect_stream_events(resp, &mut events, 1).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pod.metadata.user.name, pod_name_2);
        assert_eq!(events[0].pod.node_name, "");
    }

    #[actix_web::test]
    async fn test_update_pod_status() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let (node_name, pod_name) = add_assigned_pod(&state).await;

        let app = pod_service(&state).await;

        let payload = PodStatusUpdate {
            node_name,
            status: PodStatus::Running,
            container_statuses: vec![],
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}/status", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
    }

    #[actix_web::test]
    async fn test_update_pod_status_pod_name_not_found() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let n = Node::default();
        assert!(state.add_node(&n).await.is_ok());

        let app = pod_service(&state).await;

        let payload = PodStatusUpdate {
            node_name: n.name,
            status: PodStatus::Running,
            container_statuses: vec![],
        };
        let req = TestRequest::patch()
            .uri("/pods/made-up/status")
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_update_pod_status_node_not_found() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let pod_name = add_pod(&state).await;
        let app = pod_service(&state).await;

        let payload = PodStatusUpdate {
            node_name: "made up".to_string(),
            status: PodStatus::Running,
            container_statuses: vec![],
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}/status", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_update_pod_status_not_assigned_to_caller() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let n = Node::default();
        assert!(state.add_node(&n).await.is_ok());

        let pod_name = add_pod(&state).await;
        let app = pod_service(&state).await;

        let payload = PodStatusUpdate {
            node_name: n.name,
            status: PodStatus::Running,
            container_statuses: vec![],
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}/status", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_assign_pod() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let n = Node::default();
        assert!(state.add_node(&n).await.is_ok());

        let pod_name = add_pod(&state).await;

        let app = pod_service(&state).await;
        let payload = PodPatch {
            pod_field: PodField::NodeName,
            value: n.name,
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }

    #[actix_web::test]
    async fn test_assign_pod_invalid_node_name() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let pod_name = add_pod(&state).await;

        let app = pod_service(&state).await;
        let payload = PodPatch {
            pod_field: PodField::NodeName,
            value: "made-up".to_string(),
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[actix_web::test]
    async fn test_assign_pod_not_found() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let n = Node::default();
        assert!(state.add_node(&n).await.is_ok());

        let app = pod_service(&state).await;
        let payload = PodPatch {
            pod_field: PodField::NodeName,
            value: n.name,
        };
        let req = TestRequest::patch()
            .uri("/pods/made-up")
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_assign_pod_already_assigned() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let (node_name, pod_name) = add_assigned_pod(&state).await;

        let app = pod_service(&state).await;
        let payload = PodPatch {
            pod_field: PodField::NodeName,
            value: node_name,
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_update_pod_spec() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let pod_name = add_pod(&state).await;

        let app = pod_service(&state).await;
        let payload = PodPatch {
            pod_field: PodField::Spec,
            value: "".to_string(),
        };
        let req = TestRequest::patch()
            .uri(&format!("/pods/{}", pod_name))
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[actix_web::test]
    async fn test_create_pod() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;

        let app = pod_service(&state).await;
        let req = TestRequest::post()
            .uri("/pods")
            .set_json(PodManifest::default())
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[actix_web::test]
    async fn test_create_pod_repeat_name() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let pod_name = add_pod(&state).await;
        let mut payload = PodManifest::default();
        payload.metadata.name = pod_name;

        let app = pod_service(&state).await;
        let req = TestRequest::post()
            .uri("/pods")
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_create_pod_repeat_container_name() {
        let state = new_state_with_store(Box::new(TestStore::new())).await;
        let mut payload = PodManifest::default();
        let container = ContainerSpec::default();
        payload.spec.containers = vec![container.clone(), container];

        let app = pod_service(&state).await;
        let req = TestRequest::post()
            .uri("/pods")
            .set_json(payload)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
