use actix_web::{post, web, HttpResponse, Responder};
use crate::models::{SendRequest, ReceiveRequest};
use crate::service::MatchingService;
use std::sync::Arc;

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
