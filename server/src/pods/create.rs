use actix_web::{web, HttpResponse, Responder};
use crate::store::R8s;
use shared::models::SpecObject;
use tracing::instrument;


#[instrument(skip(state))]
pub async fn handler(
    state: web::Data<R8s>,
    body: web::Json<SpecObject>,
) -> impl Responder {
    let spec_object = body.into_inner();

    tracing::trace!("Received: {:?}", spec_object);
    state.add_object(spec_object);

    HttpResponse::Created().finish()
}
