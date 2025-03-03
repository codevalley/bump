use actix_web::{App, HttpServer};
use bump::api::{send, receive};
use bump::service::MatchingService;
use bump::config::MatchingConfig;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = MatchingConfig::from_env().unwrap_or_default();
    let service = Arc::new(MatchingService::new(Some(config)));
    
    // Start the cleanup task
    service.start_cleanup_task();
    
    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(service.clone()))
            .service(send)
            .service(receive)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
