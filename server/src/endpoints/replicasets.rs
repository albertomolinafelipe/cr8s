use crate::State;
use actix_web::{HttpResponse, Responder, web};
use shared::api::{CreateResponse, ReplicaSetManifest};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(get))
        .route("", web::post().to(create));
}

async fn get(_state: State) -> impl Responder {
    HttpResponse::NotImplemented().finish()
}

async fn create(state: State, payload: web::Json<ReplicaSetManifest>) -> impl Responder {
    let manifest = payload.into_inner();

    if manifest.metadata.name.is_none()
        || manifest.metadata.owner_reference.is_some()
        || manifest.spec.replicas < 1
    {
        return HttpResponse::BadRequest().finish();
    }

    let rs_name = manifest.metadata.name.clone().unwrap();

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
