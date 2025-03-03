//! API endpoints for the Bump service.
//! Provides HTTP endpoints for sending and receiving data between devices.
//! All endpoints use JSON for request/response bodies and follow RESTful principles.

use actix_web::{post, web, HttpResponse, Responder, ResponseError};
use crate::models::{SendRequest, ReceiveRequest};
use crate::service::MatchingService;
use std::sync::Arc;

/// Send endpoint for initiating a data transfer.
///
/// This endpoint accepts a JSON payload containing:
/// - matchingData: Criteria for matching (timestamp, location, custom key)
/// - payload: The data to be transferred
/// - ttl: Time to live for the request in milliseconds
///
/// # Returns
/// - 200 OK with match details if successful
/// - 408 Request Timeout if no match found within TTL
/// - 429 Too Many Requests if queue is full
/// - 400 Bad Request if request data is invalid
#[post("/send")]
pub async fn send(
    request: web::Json<SendRequest>,
    service: web::Data<Arc<MatchingService>>,
) -> impl Responder {
    match service.process_send(request.into_inner()).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => e.error_response(),
    }
}

/// Receive endpoint for accepting a data transfer.
///
/// This endpoint accepts a JSON payload containing:
/// - matchingData: Criteria for matching (timestamp, location, custom key)
/// - ttl: Time to live for the request in milliseconds
///
/// The receive request will be matched against pending send requests based on:
/// 1. Temporal proximity (within 500ms)
/// 2. Spatial proximity (if location provided)
/// 3. Custom key match (if provided)
///
/// # Returns
/// - 200 OK with matched data if successful
/// - 408 Request Timeout if no match found within TTL
/// - 429 Too Many Requests if queue is full
/// - 400 Bad Request if request data is invalid
#[post("/receive")]
pub async fn receive(
    request: web::Json<ReceiveRequest>,
    service: web::Data<Arc<MatchingService>>,
) -> impl Responder {
    match service.process_receive(request.into_inner()).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => e.error_response(),
    }
}
