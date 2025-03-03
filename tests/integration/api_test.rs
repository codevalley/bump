use actix_web::{test, web, App};
use bump_service::{
    api,
    models::{Location, MatchingData, SendRequest, ReceiveRequest},
    service::MatchingService,
    config::MatchingConfig,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[actix_web::test]
async fn test_send_endpoint() {
    // Initialize the service with test configuration
    let config = MatchingConfig::default();
    let service = web::Data::new(Arc::new(MatchingService::new(Some(config))));

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(service.clone())
            .service(api::send)
    ).await;

    // Create test request
    let req = test::TestRequest::post()
        .uri("/send")
        .set_json(SendRequest {
            matching_data: MatchingData {
                location: Some(Location {
                    lat: 37.7749,
                    long: -122.4194,
                }),
                timestamp: get_current_timestamp(),
                custom_key: Some("test-key".to_string()),
            },
            payload: "test payload".to_string(),
            ttl: 1000,
        });

    // Send request and check response
    let resp = test::call_service(&app, req.to_request()).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_receive_endpoint() {
    // Initialize the service with test configuration
    let config = MatchingConfig::default();
    let service = web::Data::new(Arc::new(MatchingService::new(Some(config))));

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(service.clone())
            .service(api::receive)
    ).await;

    // Create test request
    let req = test::TestRequest::post()
        .uri("/receive")
        .set_json(ReceiveRequest {
            matching_data: MatchingData {
                location: Some(Location {
                    lat: 37.7749,
                    long: -122.4194,
                }),
                timestamp: get_current_timestamp(),
                custom_key: Some("test-key".to_string()),
            },
            ttl: 1000,
        });

    // Send request and check response
    let resp = test::call_service(&app, req.to_request()).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_matching_flow() {
    // Initialize the service with test configuration
    let config = MatchingConfig::default();
    let service = web::Data::new(Arc::new(MatchingService::new(Some(config))));

    // Create test app with both endpoints
    let app = test::init_service(
        App::new()
            .app_data(service.clone())
            .service(api::send)
            .service(api::receive)
    ).await;

    // Create common test data
    let timestamp = get_current_timestamp();
    let location = Location {
        lat: 37.7749,
        long: -122.4194,
    };
    let custom_key = "test-key".to_string();
    let test_payload = "test payload".to_string();

    // Send request
    let send_req = test::TestRequest::post()
        .uri("/send")
        .set_json(SendRequest {
            matching_data: MatchingData {
                location: Some(location.clone()),
                timestamp,
                custom_key: Some(custom_key.clone()),
            },
            payload: test_payload.clone(),
            ttl: 1000,
        });

    let send_resp = test::call_service(&app, send_req.to_request()).await;
    assert!(send_resp.status().is_success());

    // Receive request (should match the send request)
    let receive_req = test::TestRequest::post()
        .uri("/receive")
        .set_json(ReceiveRequest {
            matching_data: MatchingData {
                location: Some(location),
                timestamp,
                custom_key: Some(custom_key),
            },
            ttl: 1000,
        });

    let receive_resp = test::call_service(&app, receive_req.to_request()).await;
    assert!(receive_resp.status().is_success());

    // Parse response and verify payload
    let resp_body: serde_json::Value = test::read_body_json(receive_resp).await;
    assert_eq!(resp_body["payload"], test_payload);
}

#[actix_web::test]
async fn test_queue_full() {
    // Initialize the service with a very small queue size
    let mut config = MatchingConfig::default();
    config.max_queue_size = 1;
    let service = web::Data::new(Arc::new(MatchingService::new(Some(config))));

    // Create test app
    let app = test::init_service(
        App::new()
            .app_data(service.clone())
            .service(api::send)
    ).await;

    // Helper function to create a test request
    let create_request = || test::TestRequest::post()
        .uri("/send")
        .set_json(SendRequest {
            matching_data: MatchingData {
                location: Some(Location {
                    lat: 37.7749,
                    long: -122.4194,
                }),
                timestamp: get_current_timestamp(),
                custom_key: Some("test-key".to_string()),
            },
            payload: "test payload".to_string(),
            ttl: 1000,
        });

    // First request should succeed
    let resp1 = test::call_service(&app, create_request().to_request()).await;
    assert!(resp1.status().is_success());

    // Second request should fail with 429 Too Many Requests
    let resp2 = test::call_service(&app, create_request().to_request()).await;
    assert_eq!(resp2.status().as_u16(), 429);
}
