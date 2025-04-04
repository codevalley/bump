use std::sync::Arc;
use tokio::sync::broadcast;
use time::OffsetDateTime;
use crate::queue::types::*;
use crate::queue::{DEFAULT_MAX_TIME_DIFF_MS, DEFAULT_MAX_DISTANCE_METERS};
use crate::queue::{DEFAULT_MIN_SCORE_WITHOUT_KEY, DEFAULT_MIN_SCORE_WITH_KEY, DEFAULT_CUSTOM_KEY_MATCH_BONUS};

/// In-memory implementation of RequestQueue using a single queue for both send and receive requests.
/// Uses a HashMap protected by RwLock for storing requests and a broadcast channel
/// for real-time event notifications.
///
/// This implementation has the following characteristics:
/// - Thread-safe using Arc<RwLock<HashMap>> to share state between clones
/// - Real-time notifications using tokio::sync::broadcast
/// - Configurable maximum size to prevent memory exhaustion
/// - O(1) lookup and insertion
/// - Single queue for both send and receive requests
pub struct UnifiedQueue {
    /// Thread-safe shared storage for pending requests
    pub(super) requests: Arc<parking_lot::RwLock<std::collections::HashMap<String, QueuedRequest>>>,
    /// Channel for broadcasting queue events to subscribers
    pub(super) event_tx: broadcast::Sender<RequestEvent>,
    /// Maximum number of requests allowed in queue
    pub(super) max_size: usize,
    
    // Matching configuration parameters
    /// Max time difference in milliseconds (from config)
    pub(super) max_time_diff_ms: i64,
    /// Max distance in meters for spatial matching
    pub(super) max_distance_meters: f64,
    /// Minimum score threshold without custom key
    pub(super) min_score_without_key: i32,
    /// Minimum score threshold with custom key
    pub(super) min_score_with_key: i32,
    /// Bonus points for custom key match
    pub(super) custom_key_match_bonus: i32,
}

impl Clone for UnifiedQueue {
    fn clone(&self) -> Self {
        // Clone just shares the Arc references, ensuring all clones
        // operate on the same underlying data
        Self {
            requests: self.requests.clone(),
            event_tx: self.event_tx.clone(),
            max_size: self.max_size,
            max_time_diff_ms: self.max_time_diff_ms,
            max_distance_meters: self.max_distance_meters,
            min_score_without_key: self.min_score_without_key,
            min_score_with_key: self.min_score_with_key,
            custom_key_match_bonus: self.custom_key_match_bonus,
        }
    }
}

impl UnifiedQueue {
    /// Creates a new queue with default matching parameters
    #[allow(dead_code)]
    pub fn new(event_buffer: usize, max_size: usize) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        Self {
            requests: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            event_tx: tx,
            max_size,
            max_time_diff_ms: DEFAULT_MAX_TIME_DIFF_MS,
            max_distance_meters: DEFAULT_MAX_DISTANCE_METERS,
            min_score_without_key: DEFAULT_MIN_SCORE_WITHOUT_KEY,
            min_score_with_key: DEFAULT_MIN_SCORE_WITH_KEY,
            custom_key_match_bonus: DEFAULT_CUSTOM_KEY_MATCH_BONUS,
        }
    }
    
    /// Creates a new queue with custom time difference configuration
    pub fn new_with_config(event_buffer: usize, max_size: usize, max_time_diff_ms: i64, max_distance_meters: f64) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        
        // Log the max_time_diff_ms value for debugging
        log::info!("Creating UnifiedQueue with max_time_diff_ms: {}ms", max_time_diff_ms);
        
        Self {
            requests: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            event_tx: tx,
            max_size,
            max_time_diff_ms,
            max_distance_meters: max_distance_meters,
            min_score_without_key: DEFAULT_MIN_SCORE_WITHOUT_KEY,
            min_score_with_key: DEFAULT_MIN_SCORE_WITH_KEY,
            custom_key_match_bonus: DEFAULT_CUSTOM_KEY_MATCH_BONUS,
        }
    }
    
    /// Creates a new queue with full custom matching configuration
    #[allow(dead_code)]
    pub fn new_with_full_config(
        event_buffer: usize, 
        max_size: usize,
        max_time_diff_ms: i64,
        max_distance_meters: f64,
        min_score_without_key: i32,
        min_score_with_key: i32,
        custom_key_match_bonus: i32
    ) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        Self {
            requests: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            event_tx: tx,
            max_size,
            max_time_diff_ms,
            max_distance_meters,
            min_score_without_key,
            min_score_with_key,
            custom_key_match_bonus,
        }
    }
    
    // Helper method to create a QueuedRequest with defaults
    #[allow(dead_code)]
    pub fn create_request(
        id: String,
        matching_data: crate::models::MatchingData,
        payload: Option<String>,
        expires_at: OffsetDateTime,
        request_type: RequestType,
    ) -> QueuedRequest {
        QueuedRequest {
            id,
            matching_data,
            payload,
            expires_at,
            state: RequestState::Active,
            reserved_by: None,
            request_type,
            response_tx: None,
        }
    }
}