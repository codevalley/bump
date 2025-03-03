use thiserror::Error;
use actix_web::{HttpResponse, ResponseError};

#[derive(Error, Debug)]
pub enum BumpError {
    #[error("Invalid request data: {0}")]
    ValidationError(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Queue is full")]
    QueueFull,
}

impl ResponseError for BumpError {
    fn error_response(&self) -> HttpResponse {
        match self {
            BumpError::ValidationError(msg) => {
                HttpResponse::BadRequest().json(json!({
                    "error": "validation_error",
                    "message": msg
                }))
            }
            BumpError::Timeout => {
                HttpResponse::RequestTimeout().json(json!({
                    "error": "timeout",
                    "message": "No matching bump request found within the specified time window"
                }))
            }
            BumpError::QueueFull => {
                HttpResponse::TooManyRequests().json(json!({
                    "error": "queue_full",
                    "message": "Request queue is full, please try again later"
                }))
            }
            BumpError::Internal(msg) => {
                log::error!("Internal error: {}", msg);
                HttpResponse::InternalServerError().json(json!({
                    "error": "internal_error",
                    "message": "An internal server error occurred"
                }))
            }
        }
    }
}
