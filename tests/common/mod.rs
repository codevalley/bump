use bump_service::models::{Location, MatchingData, SendRequest, ReceiveRequest};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds since UNIX epoch
pub fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Create a test location in San Francisco
pub fn test_location() -> Location {
    Location {
        lat: 37.7749,
        long: -122.4194,
    }
}

/// Create a test location slightly offset from the base location
pub fn test_location_offset(meters: f64) -> Location {
    // Approximate conversion: 1 degree latitude = 111km
    // 1 degree longitude = 111km * cos(latitude)
    let lat_offset = meters / 111_000.0;
    let long_offset = meters / (111_000.0 * (37.7749_f64.to_radians().cos()));

    Location {
        lat: 37.7749 + lat_offset,
        long: -122.4194 + long_offset,
    }
}

/// Create a test matching data struct
pub fn test_matching_data(
    location: Option<Location>,
    timestamp_offset_ms: i64,
    custom_key: Option<String>,
) -> MatchingData {
    MatchingData {
        location,
        timestamp: get_current_timestamp() + timestamp_offset_ms,
        custom_key,
    }
}

/// Create a test send request
pub fn test_send_request(
    location: Option<Location>,
    timestamp_offset_ms: i64,
    custom_key: Option<String>,
    payload: String,
    ttl: u32,
) -> SendRequest {
    SendRequest {
        matching_data: test_matching_data(location, timestamp_offset_ms, custom_key),
        payload,
        ttl,
    }
}

/// Create a test receive request
pub fn test_receive_request(
    location: Option<Location>,
    timestamp_offset_ms: i64,
    custom_key: Option<String>,
    ttl: u32,
) -> ReceiveRequest {
    ReceiveRequest {
        matching_data: test_matching_data(location, timestamp_offset_ms, custom_key),
        ttl,
    }
}
