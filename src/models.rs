use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub lat: f64,
    pub long: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRequest {
    pub matching_data: MatchingData,
    pub payload: String,
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveRequest {
    pub matching_data: MatchingData,
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResponse {
    pub status: MatchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_id: Option<String>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchStatus {
    Matched,
    Timeout,
}

fn default_ttl() -> u32 {
    500 // Default TTL of 500ms
}
