//! Error types for the Bump service.
//! Defines a comprehensive set of errors that can occur during request processing,
//! along with their HTTP response mappings.

use thiserror::Error;
use actix_web::{HttpResponse, ResponseError};
use serde_json::json;
use log;

/// Errors that can occur during request processing.
/// Each variant maps to a specific HTTP status code and includes
/// appropriate error details in the response body.
#[derive(Error, Debug)]
pub enum BumpError {
    /// Request not found in queue
    #[error("Request not found: {0}")]
    RequestNotFound(String),
    /// Returned when request validation fails (400 Bad Request)
    /// Contains a description of what validation failed
    #[error("Invalid request data: {0}")]
    ValidationError(String),
    
    /// Returned when no match is found within TTL (408 Request Timeout)
    #[error("Request timeout")]
    Timeout,
    
    /// Returned for unexpected internal errors (500 Internal Server Error)
    /// Contains internal error details (logged but not sent to client)
    #[error("Internal server error: {0}")]
    #[allow(dead_code)]
    Internal(String),

    /// Returned when request queue is at capacity (429 Too Many Requests)
    #[error("Queue is full")]
    QueueFull,
    
    /// Returned when a request is not found in the queue
    #[error("Request not found: {0}")]
    NotFound(String),
}

/// Implementation of actix_web::ResponseError for BumpError.
/// Maps each error variant to an appropriate HTTP response with
/// consistent error format in the JSON body.
///
/// Error Response Format:
/// ```json
/// {
///     "error": "error_type",
///     "message": "Human readable error message"
/// }
/// ```
impl ResponseError for BumpError {
    fn error_response(&self) -> HttpResponse {
        match self {
            &BumpError::RequestNotFound(ref msg) => {
                HttpResponse::NotFound().json(json!({
                    "error": "request_not_found",
                    "message": msg
                }))
            },
            // 400 Bad Request - Client provided invalid data
            BumpError::ValidationError(msg) => {
                HttpResponse::BadRequest().json(json!({
                    "error": "validation_error",
                    "message": msg
                }))
            }
            // 408 Request Timeout - No match found within TTL
            BumpError::Timeout => {
                HttpResponse::RequestTimeout().json(json!({
                    "error": "timeout",
                    "message": "No matching bump request found within the specified time window"
                }))
            }
            // 429 Too Many Requests - Queue is at capacity
            BumpError::QueueFull => {
                HttpResponse::TooManyRequests().json(json!({
                    "error": "queue_full",
                    "message": "Request queue is full, please try again later"
                }))
            }
            // 404 Not Found - Request not found in queue
            BumpError::NotFound(msg) => {
                log::warn!("Request not found: {}", msg);
                HttpResponse::NotFound().json(json!({
                    "error": "not_found",
                    "message": "The requested item could not be found"
                }))
            }
            // 500 Internal Server Error - Unexpected internal error
            BumpError::Internal(msg) => {
                // Log internal error details but don't expose them to client
                log::error!("Internal error: {}", msg);
                HttpResponse::InternalServerError().json(json!({
                    "error": "internal_error",
                    "message": "An internal server error occurred"
                }))
            }
        }
    }
}
