use actix_web::web;

mod register;
mod get;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(web::scope("/nodes")
            .route("", web::post().to(register::handler))
            .route("", web::get().to(get::handler)));
}

