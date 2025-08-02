use actix_web::HttpResponse as Http;
use std::fmt;

/// Represents errors that can occur in the `Store` implementation.
pub enum StoreError {
    /// Input data is in the wrong format
    WrongFormat(String),
    /// Resource already exists or violates uniqueness constraint.
    Conflict(String),
    /// Requested resource was not found.
    NotFound(String),
    /// Referenced resource is invalid or missing
    InvalidReference(String),
    /// An unexpected error occurred in logic or state not covered by other cases.
    UnexpectedError(String),
    /// Error from an external storage backend
    BackendError(String),
}

impl StoreError {
    /// Maps the error to an appropriate HTTP response.
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
