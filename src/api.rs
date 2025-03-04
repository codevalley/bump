//! API endpoints for the Bump service.
//! Provides HTTP endpoints for sending and receiving data between devices.
//! All endpoints use JSON for request/response bodies and follow RESTful principles.

use actix_web::{post, get, web, HttpResponse, Responder, ResponseError};
use crate::models::{SendRequest, ReceiveRequest, BumpRequest};
use crate::service::MatchingService;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    service: Option<web::Data<Arc<MatchingService>>>,
) -> impl Responder {
    // First check if service is available
    if service.is_none() {
        log::error!("MatchingService not available in send endpoint");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "message": "Service dependency not available"
        }));
    }
    
    // Service is available, proceed with request
    let service = service.unwrap();
    let request_inner = request.into_inner();
    
    // Add debug logging for the request
    if let Some(key) = &request_inner.matching_data.custom_key {
        log::info!("API: Processing send request with key: {}, timestamp: {}", 
                  key, request_inner.matching_data.timestamp);
    }
    
    match service.process_send(request_inner).await {
        Ok(response) => {
            log::info!("API: Send request matched successfully: {:?}", response);
            HttpResponse::Ok().json(response)
        },
        Err(e) => {
            log::warn!("API: Send request failed: {:?}", e);
            e.error_response()
        },
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
    service: Option<web::Data<Arc<MatchingService>>>,
) -> impl Responder {
    // First check if service is available
    if service.is_none() {
        log::error!("MatchingService not available in receive endpoint");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "message": "Service dependency not available"
        }));
    }
    
    // Service is available, proceed with request
    let service = service.unwrap();
    let request_inner = request.into_inner();
    
    // Add debug logging for the request
    if let Some(key) = &request_inner.matching_data.custom_key {
        log::info!("API: Processing receive request with key: {}, timestamp: {}", 
                  key, request_inner.matching_data.timestamp);
    }
    
    match service.process_receive(request_inner).await {
        Ok(response) => {
            log::info!("API: Receive request matched successfully: {:?}", response);
            HttpResponse::Ok().json(response)
        },
        Err(e) => {
            log::warn!("API: Receive request failed: {:?}", e);
            e.error_response()
        },
    }
}

/// Health check endpoint for monitoring service status.
///
/// Returns various metrics and status information for the service:
/// - Service status (ok, degraded, error)
/// - Version information
/// - Uptime
/// - Queue sizes and capacities
/// - Matching statistics
///
/// # Returns
/// - 200 OK with health status JSON
#[get("/health")]
pub async fn health(service: Option<web::Data<Arc<MatchingService>>>) -> impl Responder {
    // First check if service is available at all
    if service.is_none() {
        log::error!("MatchingService not available in health endpoint");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "version": env!("CARGO_PKG_VERSION"),
            "message": "Service dependency not available",
            "error": "Service data was not properly injected"
        }));
    }
    
    // Service is available but might still fail
    let service = service.unwrap();
    
    // Try to get health status, with error handling
    let health_status = match service.get_health_status() {
        Ok(status) => status,
        Err(e) => {
            // Log the error
            log::error!("Failed to get health status: {:?}", e);
            
            // Return a degraded status
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "degraded",
                "version": env!("CARGO_PKG_VERSION"),
                "message": "Error retrieving health data",
                "error": format!("{:?}", e)
            }));
        }
    };
    
    HttpResponse::Ok().json(health_status)
}

// Root-level health endpoint for platform health checks
// This version doesn't rely on MatchingService to work
#[get("/")]
pub async fn root_health() -> impl Responder {
    // Simple response with minimal information
    let simple_health = serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "message": "Bump service is running"
    });
    HttpResponse::Ok().json(simple_health)
}

/// Timestamp endpoint for providing current server time.
///
/// Returns the current server time in milliseconds since epoch.
/// Clients can use this to synchronize their timestamps with the server.
///
/// # Returns
/// - Plain text response with the timestamp as a number
#[get("/timestamp")]
pub async fn timestamp() -> impl Responder {
    // Get current time in milliseconds since epoch
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    
    // Return as plain text
    HttpResponse::Ok().body(timestamp.to_string())
}

/// Unified bump endpoint for symmetric data exchange.
///
/// This endpoint accepts a JSON payload containing:
/// - matchingData: Criteria for matching (timestamp, location, custom key)
/// - payload: Optional data to share with the matched request
/// - ttl: Time to live for the request in milliseconds
///
/// The bump endpoint handles both sending and receiving in a single request.
/// When two compatible bump requests meet, they exchange their payloads.
///
/// # Returns
/// - 200 OK with match details if successful
/// - 408 Request Timeout if no match found within TTL
/// - 429 Too Many Requests if queue is full
/// - 400 Bad Request if request data is invalid
#[post("/bump")]
pub async fn bump(
    request: web::Json<BumpRequest>,
    service: Option<web::Data<Arc<MatchingService>>>,
) -> impl Responder {
    // First check if service is available
    if service.is_none() {
        log::error!("MatchingService not available in bump endpoint");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "status": "error",
            "message": "Service dependency not available"
        }));
    }
    
    // Service is available, proceed with request
    let service = service.unwrap();
    let request_inner = request.into_inner();
    
    // Add debug logging for the request
    if let Some(key) = &request_inner.matching_data.custom_key {
        log::info!("API: Processing bump request with key: {}, timestamp: {}", 
                  key, request_inner.matching_data.timestamp);
    }
    
    log::info!("API: Bump request received with payload: {}", 
              if request_inner.payload.is_some() { "yes" } else { "no" });
    
    match service.process_bump(request_inner).await {
        Ok(response) => {
            log::info!("API: Bump request matched successfully: {:?}", response);
            HttpResponse::Ok().json(response)
        },
        Err(e) => {
            log::warn!("API: Bump request failed: {:?}", e);
            e.error_response()
        },
    }
}
