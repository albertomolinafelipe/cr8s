//! ReplicaSet
//!
//! ## Routes
//! - `GET    /replicasets`                    — List or watch replicasets
//! - `POST   /replicasets`                    — Create a new replicaset

use crate::state::State;
use actix_web::{
    HttpResponse, Responder,
    web::{self, Bytes},
};
use serde::Deserialize;
use shared::api::{CreateResponse, EventType, ReplicaSetEvent, ReplicaSetManifest};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("", web::post().to(create));
}

#[derive(Deserialize)]
pub struct ReplicaSetQuery {
    watch: Option<bool>,
}

/// List or watch replicasets
///
/// # Arguments
/// - `query`: Query parameters:
///    - `watch` (bool, optional): If true, opens a watch stream of node events.
///    - TODO filter or get by name
///
/// # Returns
/// - 200 list of nodes or stream of node events
async fn get(state: State, query: web::Query<ReplicaSetQuery>) -> impl Responder {
    let replicasets = state.get_replicasets().await;
    if query.watch.unwrap_or(false) {
        // Watch mode
        let mut rx = state.replicaset_tx.subscribe();
        let stream = async_stream::stream! {
            for rs in replicasets {
                let event = ReplicaSetEvent {
                    replicaset: rs,
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
        HttpResponse::Ok().json(&replicasets)
    }
}

/// Create a new Replicaset
///
/// # Arguments
/// - `payload`: ReplicaSetManifest
///
/// # Returns
/// - 201: Success
/// - 400: Replicas < 1 or set owner reference
/// - 409: Duplicate name
async fn create(state: State, payload: web::Json<ReplicaSetManifest>) -> impl Responder {
    let manifest = payload.into_inner();

    if manifest.metadata.owner_reference.is_some() || manifest.spec.replicas < 1 {
        return HttpResponse::BadRequest().finish();
    }

    let rs_name = manifest.metadata.name.clone();

    if state.cache.replicaset_name_exists(&rs_name) {
        return HttpResponse::Conflict().body("Duplicate replicaset name");
    };

    match state
        .add_replicaset(manifest.spec, manifest.metadata.into())
        .await
    {
        Ok(id) => {
            tracing::info!(
                name=%rs_name,
                "Replicaset created"
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
                "Could not create replicaset"
            );
            err.to_http_response()
        }
    }
}

#[cfg(test)]
mod tests {

    //!  GET
    //!  - test_get_replicasets
    //!  - test_get_replicasets_watch
    //!
    //!  CREATE
    //!  - test_create_replicaset

    use crate::endpoints::helpers::collect_stream_events;
    use crate::state::{ApiServerState, test_store::TestStore};

    use super::*;
    use actix_web::body::BoxBody;
    use actix_web::dev::Service;
    use actix_web::{
        App,
        http::StatusCode,
        test::{self, TestRequest, call_service, init_service, read_body_json},
    };
    use shared::models::metadata::OwnerReference;
    use shared::models::replicaset::ReplicaSet;

    async fn replicaset_service(
        state: &State,
    ) -> impl Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<BoxBody>,
        Error = actix_web::Error,
    > {
        init_service(
            App::new()
                .app_data(state.clone())
                .route("/replicasets", web::get().to(get))
                .route("/replicasets", web::post().to(create)),
        )
        .await
    }

    // --- Get replicasets ---

    #[actix_web::test]
    async fn test_get_replicasets() {
        let test_store = TestStore::new();
        let rs = ReplicaSet::default();
        test_store.replicasets.insert(rs.metadata.id.clone(), rs);
        let state = ApiServerState::new_with_store(Box::new(test_store)).await;

        let app = replicaset_service(&state).await;
        let req = TestRequest::get().uri("/replicasets").to_request();
        let res = call_service(&app, req).await;
        assert!(res.status().is_success());
        let reps: Vec<ReplicaSet> = read_body_json(res).await;
        assert_eq!(reps.len(), 1, "Replicaset list should have one item");
    }

    #[actix_web::test]
    async fn test_get_replicaset_watch() {
        let test_store = TestStore::new();
        let rs1 = ReplicaSet::default();
        let rs2 = ReplicaSet::default();

        test_store
            .replicasets
            .insert(rs1.metadata.id.clone(), rs1.clone());
        let state = ApiServerState::new_with_store(Box::new(test_store)).await;

        let app = replicaset_service(&state).await;

        let req = test::TestRequest::get()
            .uri("/replicasets?watch=true")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(
            state
                .add_replicaset(rs2.spec, rs2.metadata.into())
                .await
                .is_ok()
        );
        assert!(resp.status().is_success());

        let mut events: Vec<ReplicaSetEvent> = Vec::new();
        collect_stream_events(resp, &mut events, 2).await;

        assert_eq!(events.len(), 2);
        assert_ne!(
            events[0].replicaset.metadata.id,
            events[1].replicaset.metadata.id
        );
    }

    // --- Create replicasets ---

    #[actix_web::test]
    async fn test_create_replicaset_bad_format() {
        let test_store = TestStore::new();
        let mut manifest = ReplicaSetManifest::default();
        manifest.metadata.owner_reference = Some(OwnerReference::default());
        manifest.spec.replicas = 0;

        let state = ApiServerState::new_with_store(Box::new(test_store)).await;

        let app = replicaset_service(&state).await;

        let req = TestRequest::post()
            .uri("/replicasets")
            .set_json(&manifest)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_create_replicaset_repeat_name() {
        let test_store = TestStore::new();
        let state = ApiServerState::new_with_store(Box::new(test_store)).await;
        let rs1 = ReplicaSet::default();
        let mut manifest = ReplicaSetManifest::default();
        manifest.metadata.name = rs1.metadata.name.clone();

        let _ = state.add_replicaset(rs1.spec, rs1.metadata).await;

        let app = replicaset_service(&state).await;

        let req = TestRequest::post()
            .uri("/replicasets")
            .set_json(&manifest)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[actix_web::test]
    async fn test_create_replicaset() {
        let test_store = TestStore::new();
        let state = ApiServerState::new_with_store(Box::new(test_store)).await;
        let manifest = ReplicaSetManifest::default();

        let app = replicaset_service(&state).await;

        let req = TestRequest::post()
            .uri("/replicasets")
            .set_json(&manifest)
            .to_request();
        let res = call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CREATED);
    }
}
