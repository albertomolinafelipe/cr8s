use actix_web::HttpResponse as Http;
use std::fmt;

pub enum StoreError {
    WrongFormat(String),
    Conflict(String),
    NotFound(String),
    InvalidReference(String),
    UnexpectedError(String),
    BackendError(String),
}

impl StoreError {
    pub fn to_http_response(&self) -> Http {
        match self {
            StoreError::WrongFormat(msg) => Http::BadRequest().body(msg.clone()),
            StoreError::Conflict(msg) => Http::Conflict().body(msg.clone()),
            StoreError::NotFound(msg) => Http::NotFound().body(msg.clone()),
            StoreError::InvalidReference(msg) => Http::UnprocessableEntity().body(msg.clone()),
            StoreError::UnexpectedError(_) | StoreError::BackendError(_) => {
                Http::InternalServerError().body("Unexpected error")
            }
        }
    }
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::WrongFormat(msg) => write!(f, "Wrong format: {}", msg),
            StoreError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            StoreError::NotFound(msg) => write!(f, "Not found error: {}", msg),
            StoreError::InvalidReference(msg) => write!(f, "Invalid reference error: {}", msg),
            StoreError::UnexpectedError(msg) => write!(f, "Unexpected error: {}", msg),
            StoreError::BackendError(msg) => write!(f, "Backend error: {}", msg),
        }
    }
}
