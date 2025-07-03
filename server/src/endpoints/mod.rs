mod nodes;
mod pods;

use actix_web::web::{self, scope};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(scope("/nodes").configure(nodes::config))
        .service(scope("/pods").configure(pods::config));
}
