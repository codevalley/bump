//! Data models for the Bump service.
//! Defines the core data structures used for request/response handling
//! and matching logic.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;


/// Geographic location represented by latitude and longitude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Latitude in degrees (-90 to 90)
    pub lat: f64,
    /// Longitude in degrees (-180 to 180)
    pub long: f64,
}

/// Core matching criteria used to pair send and receive requests.
/// All fields are optional except timestamp to support different
/// matching strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingData {
    /// Optional geographic location for spatial matching
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Unix timestamp in milliseconds when the request was created
    pub timestamp: i64,
    /// Optional custom key for exact matching
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_key: Option<String>,
}

/// Request to send data to a matching receive request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRequest {
    /// Criteria used to match this request with a receive request
    pub matching_data: MatchingData,
    /// Data payload to transfer
    pub payload: String,
    /// Time-to-live in milliseconds (defaults to 500ms)
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

/// Request to receive data from a matching send request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveRequest {
    /// Criteria used to match this request with a send request
    pub matching_data: MatchingData,
    /// Time-to-live in milliseconds (defaults to 500ms)
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

/// Unified request structure for the /bump endpoint
/// This combines send and receive functionality into a single model
/// with an optional payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BumpRequest {
    /// Criteria used to match this request with other bump requests
    pub matching_data: MatchingData,
    /// Optional data payload to share with the matched request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Time-to-live in milliseconds (defaults to 500ms)
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

/// Response containing the result of a match attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResponse {
    /// Status of the match attempt
    pub status: MatchStatus,
    /// ID of the sending request if matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_id: Option<String>,
    /// ID of the receiving request if matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_id: Option<String>,
    /// Unix timestamp in milliseconds when the match occurred
    pub timestamp: i64,
    /// Data payload if this was a receive request that matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// Optional message providing additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Status of a match attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchStatus {
    /// Successfully matched with another request
    Matched,
    /// No match found within TTL
    Timeout,
}

/// Default time-to-live for requests in milliseconds.
/// After this duration, unmatched requests are removed from the queue.
fn default_ttl() -> u32 {
    500 // Default TTL of 500ms
}

/// Health status response for monitoring the Bump service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall service health status
    pub status: String,
    /// Current version of the service
    pub version: String,
    /// When the service was started
    pub uptime_seconds: u64,
    /// System metrics
    pub metrics: HashMap<String, u64>,
    /// Queue statistics
    pub queue_stats: QueueStats,
}

/// Statistics about the request queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Number of send requests currently in queue
    pub send_queue_size: usize,
    /// Number of receive requests currently in queue
    pub receive_queue_size: usize,
    /// Number of matches made since startup
    pub matches_count: u64,
    /// Number of expired requests since startup
    pub expired_count: u64,
    /// Current match rate (matches per second)
    pub match_rate: f64,
}
