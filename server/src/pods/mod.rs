use actix_web::web;

mod get;
mod create;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(web::scope("/pods")
            .route("", web::get().to(get::handler))
            .route("", web::post().to(create::handler)));
}
