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
