use bump_service::{
    service::MatchingService,
    models::{Location, MatchingData, SendRequest, ReceiveRequest, MatchStatus},
    config::MatchingConfig,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
async fn test_exact_match() {
    let service = MatchingService::new(None); // Use default config
    let timestamp = get_current_timestamp();
    let location = Location { lat: 37.7749, long: -122.4194 };
    let custom_key = "test-key".to_string();
    let payload = "test payload".to_string();

    // Send request
    let send_result = service.process_send(SendRequest {
        matching_data: MatchingData {
            location: Some(location.clone()),
            timestamp,
            custom_key: Some(custom_key.clone()),
        },
        payload: payload.clone(),
        ttl: 1000,
    }).await;
    assert!(send_result.is_ok());

    // Receive request with exact same criteria
    let receive_result = service.process_receive(ReceiveRequest {
        matching_data: MatchingData {
            location: Some(location),
            timestamp,
            custom_key: Some(custom_key),
        },
        ttl: 1000,
    }).await;

    assert!(receive_result.is_ok());
    let response = receive_result.unwrap();
    assert_eq!(response.status, MatchStatus::Matched);
    assert_eq!(response.payload.unwrap(), payload);
}

#[tokio::test]
async fn test_temporal_match() {
    let mut config = MatchingConfig::default();
    config.max_time_diff_ms = 1000; // Allow 1 second difference
    let service = MatchingService::new(Some(config));

    let timestamp = get_current_timestamp();
    let payload = "test payload".to_string();

    // Send request
    let send_result = service.process_send(SendRequest {
        matching_data: MatchingData {
            location: None, // No location for pure temporal matching
            timestamp,
            custom_key: None,
        },
        payload: payload.clone(),
        ttl: 2000,
    }).await;
    assert!(send_result.is_ok());

    // Wait 500ms
    sleep(Duration::from_millis(500)).await;

    // Receive request with slightly different timestamp
    let receive_result = service.process_receive(ReceiveRequest {
        matching_data: MatchingData {
            location: None,
            timestamp: timestamp + 500, // 500ms later
            custom_key: None,
        },
        ttl: 2000,
    }).await;

    assert!(receive_result.is_ok());
    let response = receive_result.unwrap();
    assert_eq!(response.status, MatchStatus::Matched);
    assert_eq!(response.payload.unwrap(), payload);
}

#[tokio::test]
async fn test_spatial_match() {
    let mut config = MatchingConfig::default();
    config.max_distance_meters = 10.0; // Allow 10 meter difference
    let service = MatchingService::new(Some(config));

    let timestamp = get_current_timestamp();
    let payload = "test payload".to_string();

    // Base location
    let location1 = Location { lat: 37.7749, long: -122.4194 };
    // Location ~5 meters away
    let location2 = Location { lat: 37.774904, long: -122.419395 };

    // Send request
    let send_result = service.process_send(SendRequest {
        matching_data: MatchingData {
            location: Some(location1),
            timestamp,
            custom_key: None,
        },
        payload: payload.clone(),
        ttl: 1000,
    }).await;
    assert!(send_result.is_ok());

    // Receive request with slightly different location
    let receive_result = service.process_receive(ReceiveRequest {
        matching_data: MatchingData {
            location: Some(location2),
            timestamp,
            custom_key: None,
        },
        ttl: 1000,
    }).await;

    assert!(receive_result.is_ok());
    let response = receive_result.unwrap();
    assert_eq!(response.status, MatchStatus::Matched);
    assert_eq!(response.payload.unwrap(), payload);
}

#[tokio::test]
async fn test_timeout() {
    let service = MatchingService::new(None);
    let timestamp = get_current_timestamp();
    let payload = "test payload".to_string();

    // Send request with very short TTL
    let send_result = service.process_send(SendRequest {
        matching_data: MatchingData {
            location: None,
            timestamp,
            custom_key: None,
        },
        payload: payload.clone(),
        ttl: 100, // 100ms TTL
    }).await;
    assert!(send_result.is_ok());

    // Wait longer than TTL
    sleep(Duration::from_millis(200)).await;

    // Try to receive
    let receive_result = service.process_receive(ReceiveRequest {
        matching_data: MatchingData {
            location: None,
            timestamp,
            custom_key: None,
        },
        ttl: 100,
    }).await;

    assert!(receive_result.is_ok());
    let response = receive_result.unwrap();
    assert_eq!(response.status, MatchStatus::Timeout);
    assert!(response.payload.is_none());
}

#[tokio::test]
async fn test_queue_cleanup() {
    let mut config = MatchingConfig::default();
    config.cleanup_interval_ms = 100; // Cleanup every 100ms
    let service = MatchingService::new(Some(config));

    let timestamp = get_current_timestamp();
    let payload = "test payload".to_string();

    // Send multiple requests with short TTL
    for i in 0..5 {
        let send_result = service.process_send(SendRequest {
            matching_data: MatchingData {
                location: None,
                timestamp,
                custom_key: Some(format!("key-{}", i)),
            },
            payload: payload.clone(),
            ttl: 100, // 100ms TTL
        }).await;
        assert!(send_result.is_ok());
    }

    // Wait for cleanup
    sleep(Duration::from_millis(300)).await;

    // Try to match with one of the expired requests
    let receive_result = service.process_receive(ReceiveRequest {
        matching_data: MatchingData {
            location: None,
            timestamp,
            custom_key: Some("key-1".to_string()),
        },
        ttl: 1000,
    }).await;

    assert!(receive_result.is_ok());
    let response = receive_result.unwrap();
    assert_eq!(response.status, MatchStatus::Timeout);
}

#[tokio::test]
async fn test_queue_size_limit() {
    let mut config = MatchingConfig::default();
    config.max_queue_size = 2; // Very small queue size
    let service = MatchingService::new(Some(config));

    let timestamp = get_current_timestamp();
    let payload = "test payload".to_string();

    // Fill up the queue
    for i in 0..2 {
        let send_result = service.process_send(SendRequest {
            matching_data: MatchingData {
                location: None,
                timestamp,
                custom_key: Some(format!("key-{}", i)),
            },
            payload: payload.clone(),
            ttl: 1000,
        }).await;
        assert!(send_result.is_ok());
    }

    // Try to add one more request
    let send_result = service.process_send(SendRequest {
        matching_data: MatchingData {
            location: None,
            timestamp,
            custom_key: Some("key-overflow".to_string()),
        },
        payload: payload.clone(),
        ttl: 1000,
    }).await;

    assert!(send_result.is_err());
}
