//! Main entry point for the Bump service.
//! Sets up the HTTP server, configures logging, and initializes the service with
//! environment-based configuration.

use actix_web::{web, App, HttpServer}; 
use env_logger;
use log;

// Internal module declarations
mod api;      // HTTP endpoint handlers
mod models;   // Data structures and types
mod service;  // Core business logic
mod error;    // Error types and handling
mod config;   // Configuration management
mod queue;    // Request queue implementation

use service::MatchingService;
use config::MatchingConfig;

/// Main entry point for the Bump service.
/// 
/// # Server Configuration
/// - Binds to 127.0.0.1:8080
/// - All endpoints are under the /bump prefix
/// - Supports /send and /receive endpoints
/// 
/// # Environment Variables
/// Configuration can be customized via environment variables:
/// - BUMP_MAX_QUEUE_SIZE: Maximum number of requests in queue (default: 1000)
/// - BUMP_MAX_DISTANCE_METERS: Maximum matching distance (default: 10)
/// - BUMP_MAX_TIME_DIFF_MS: Maximum time difference for matching (default: 500)
/// - BUMP_DEFAULT_TTL_MS: Default request TTL (default: 500)
/// 
/// # Error Handling
/// - Uses env_logger for logging
/// - Returns std::io::Error for server startup issues
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging with env_logger
    // Log level can be set via RUST_LOG environment variable
    env_logger::init();
    
    log::info!("Starting Bump service...");
    
    // Load configuration from environment variables
    // Falls back to defaults if env vars not set
    let config = MatchingConfig::from_env_or_default();
    log::info!("Starting Bump service with configuration: {:?}", config);
    
    // Initialize the matching service with configuration
    // Wrapped in Arc for thread-safe sharing
    let service = web::Data::new(MatchingService::new(Some(config)));

    // Configure and start the HTTP server
    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT environment variable must be a valid port number");
    
    log::info!("Starting server on port {}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(service.clone())  // Share service state across workers
            .service(
                web::scope("/bump")     // All endpoints under /bump prefix
                    .service(api::send)    // POST /bump/send
                    .service(api::receive) // POST /bump/receive
                    .service(api::health)  // GET /bump/health
            )
            // Register root-level health endpoint for platform health checks
            .service(api::root_health)
    })
    .bind(("0.0.0.0", port))?  // Bind to all interfaces with dynamic port
    .run()                     // Start the server
    .await
}
