use actix_web::{web, App, HttpServer};
mod api;
mod models;
mod service;
mod error;
mod config;

use service::MatchingService;
use config::MatchingConfig;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    
    log::info!("Starting Bump service...");
    
    // Load configuration from environment or use defaults
    let config = MatchingConfig::from_env_or_default();
    log::info!("Starting Bump service with configuration: {:?}", config);
    
    let service = web::Data::new(MatchingService::new(Some(config)));

    HttpServer::new(move || {
        App::new()
            .app_data(service.clone())
            .service(
                web::scope("/bump")
                    .service(api::send)
                    .service(api::receive)
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
