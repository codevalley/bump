use tokio::sync::broadcast;
use time::OffsetDateTime;
use crate::models::MatchingData;
use crate::error::BumpError;

/// Default time difference between requests in milliseconds
const DEFAULT_MAX_TIME_DIFF_MS: i64 = 500;

/// Default score threshold without custom key match
const DEFAULT_MIN_SCORE_WITHOUT_KEY: i32 = 150;

/// Default score threshold with custom key match
const DEFAULT_MIN_SCORE_WITH_KEY: i32 = 100;

/// Default custom key match score bonus
const DEFAULT_CUSTOM_KEY_MATCH_BONUS: i32 = 200;

/// Default maximum distance for matching (in meters)
pub const DEFAULT_MAX_DISTANCE_METERS: f64 = 5.0;

/// Earth radius in meters (standard value)
pub const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

/// A request stored in the queue awaiting a match.
/// Each request has a unique ID, matching criteria, optional payload,
/// and an expiration time.
#[derive(Debug)]
pub struct QueuedRequest {
    /// Unique identifier for this request
    pub id: String,
    /// Criteria used for matching (timestamp, location, custom key)
    pub matching_data: MatchingData,
    /// Optional payload (present for send requests, None for receive)
    pub payload: Option<String>,
    /// When this request should be removed from queue
    pub expires_at: OffsetDateTime,
    /// State of the request in the matching process
    pub state: RequestState,
    /// ID of the request that has reserved this request (if any)
    pub reserved_by: Option<String>,
    /// The request type (send or receive)
    pub request_type: RequestType,
    /// Channel to notify the waiting task when match is found
    pub response_tx: Option<tokio::sync::oneshot::Sender<MatchResult>>,
}

impl Clone for QueuedRequest {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            matching_data: self.matching_data.clone(),
            payload: self.payload.clone(),
            expires_at: self.expires_at,
            state: self.state,
            reserved_by: self.reserved_by.clone(),
            request_type: self.request_type,
            response_tx: None, // Don't clone the channel - it's not meaningiful to clone
        }
    }
}

/// The result of a match operation
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// ID of the matching request
    pub matched_with: String,
    /// Payload from the other request (if any)
    pub payload: Option<String>,
    /// When the match was made
    pub timestamp: i64,
}

/// Type of request (send or receive)
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum RequestType {
    /// Send request with payload
    Send,
    /// Receive request without payload
    Receive,
}

/// Represents the state of a request in the matching process
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum RequestState {
    /// Request is active and available for matching
    Active,
    /// Request has been reserved for matching but not yet confirmed
    #[allow(dead_code)]
    Reserved,
    /// Request has been confirmed for matching and will be removed soon
    Matched,
}

/// Event emitted when the queue state changes.
/// These events are broadcast to all subscribers and used for real-time matching.
#[derive(Clone, Debug)]
pub struct RequestEvent {
    /// The request involved in this event
    #[allow(dead_code)]
    pub request: QueuedRequest,
    /// Type of event (Added, Matched, Expired, Removed)
    pub event_type: RequestEventType,
}

#[derive(Clone, Debug)]
pub enum RequestEventType {
    Added,
    // Signals that a request has been matched with another request
    Matched(#[allow(dead_code)] String), // ID of the matching request
    // Signals that a request has been reserved for matching (phase 1 of two-phase matching)
    #[allow(dead_code)]
    Reserved(String), // ID of the reserving request
    // Signals that a request has been confirmed for matching (phase 2 of two-phase matching)
    #[allow(dead_code)]
    Confirmed(String), // ID of the confirming request
    Expired,
    Removed,
}

/// Trait defining the interface for a unified request queue.
/// This abstraction allows for different queue implementations (memory, Redis, etc.)
/// while maintaining the same matching behavior.
///
/// All implementations must be thread-safe and support async operations.
#[async_trait::async_trait]
pub trait RequestQueue: Send + Sync + 'static {
    /// Add a new request to the queue
    async fn add_request(&self, request: QueuedRequest) -> Result<Option<MatchResult>, BumpError>;
    
    /// Remove a request from the queue
    async fn remove_request(&self, request_id: &str) -> Result<(), BumpError>;
    
    /// Subscribe to queue events
    fn subscribe(&self) -> broadcast::Receiver<RequestEvent>;
    
    /// Clean up expired requests
    async fn cleanup_expired(&self) -> Result<(), BumpError>;
    
    /// Get the current size of the queue
    fn size(&self) -> usize;
    
    /// Get the maximum size of the queue
    #[allow(unused)]
    fn capacity(&self) -> usize;
    
    /// Get the current size of the queue with error handling
    fn size_safe(&self) -> Result<usize, String>;
    
    /// Get the maximum size of the queue with error handling
    fn capacity_safe(&self) -> Result<usize, String>;
    
    /// Wait for a match for the given request
    /// 
    /// Note: With the unified queue implementation, this method is deprecated.
    /// Applications should now create a channel when adding the request and
    /// wait on that channel directly.
    #[allow(unused)]
    async fn wait_for_match(&self, request_id: &str, ttl_ms: u64) -> Result<Option<MatchResult>, BumpError>;
}

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
    requests: std::sync::Arc<parking_lot::RwLock<std::collections::HashMap<String, QueuedRequest>>>,
    /// Channel for broadcasting queue events to subscribers
    event_tx: broadcast::Sender<RequestEvent>,
    /// Maximum number of requests allowed in queue
    max_size: usize,
    
    // Matching configuration parameters
    /// Max time difference in milliseconds (from config)
    max_time_diff_ms: i64,
    /// Max distance in meters for spatial matching
    max_distance_meters: f64,
    /// Minimum score threshold without custom key
    min_score_without_key: i32,
    /// Minimum score threshold with custom key
    min_score_with_key: i32,
    /// Bonus points for custom key match
    custom_key_match_bonus: i32,
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
            requests: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
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
    pub fn new_with_config(event_buffer: usize, max_size: usize, max_time_diff_ms: i64) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        
        // Log the max_time_diff_ms value for debugging
        log::info!("Creating UnifiedQueue with max_time_diff_ms: {}ms", max_time_diff_ms);
        
        Self {
            requests: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            event_tx: tx,
            max_size,
            max_time_diff_ms,
            max_distance_meters: DEFAULT_MAX_DISTANCE_METERS,
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
            requests: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
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
        matching_data: MatchingData,
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
    
    // Helper method to find a match for a request in the queue
    // Returns just the ID of the matched request, not the request itself
    fn find_matching_request(&self, request: &QueuedRequest) -> Option<String> {
        let requests = self.requests.read();
        let now = OffsetDateTime::now_utc();
        
        // Log information about the request we're trying to match
        log::info!("Finding match for request ID: {}, type: {:?}, has_channel: {}", 
                 request.id, request.request_type, request.response_tx.is_some());
        
        if let Some(key) = &request.matching_data.custom_key {
            log::info!("Request {} has custom key: {}, timestamp: {}", 
                     request.id, key, request.matching_data.timestamp);
        }
        
        let opposite_type = match request.request_type {
            RequestType::Send => RequestType::Receive,
            RequestType::Receive => RequestType::Send,
        };
        
        log::debug!("Looking for requests of type {:?}", opposite_type);
        
        // Debug dump all potential match candidates
        log::debug!("Potential match candidates:");
        for (id, r) in requests.iter()
            .filter(|(_, r)| r.expires_at > now)
            .filter(|(_, r)| r.state == RequestState::Active)
            .filter(|(_, r)| r.request_type == opposite_type)
        {
            log::debug!("  Candidate: id={}, type={:?}, key={:?}, timestamp={}, channel={}", 
                      id, r.request_type, r.matching_data.custom_key, r.matching_data.timestamp, r.response_tx.is_some());
        }
        
        // Find the best match based on score
        let matched_id = requests.iter()
            // Only consider requests that haven't expired
            .filter(|(_, r)| r.expires_at > now)
            // Only consider active requests
            .filter(|(_, r)| r.state == RequestState::Active)
            // Only consider requests of the opposite type
            .filter(|(_, r)| r.request_type == opposite_type)
            // Calculate match score
            .filter_map(|(id, r)| {
                // Debug logging for key matching
                if request.matching_data.custom_key.is_some() && r.matching_data.custom_key.is_some() {
                    let key1 = request.matching_data.custom_key.as_ref().unwrap();
                    let key2 = r.matching_data.custom_key.as_ref().unwrap();
                    log::info!("Comparing keys: {} vs {} = {}", key1, key2, key1 == key2);
                }
                
                let score = self.calculate_match_score(request, r);
                
                // Just return id and score - we'll get the actual request by ID in atomic_match
                score.map(|score| (id.clone(), score))
            })
            .max_by_key(|(_, score)| *score)
            .map(|(id, _)| id);
        
        if let Some(ref id) = matched_id {
            log::debug!("Found match with ID: {}, has_channel: {}", 
                      id, requests.get(id).map_or(false, |r| r.response_tx.is_some()));
        }
            
        matched_id
    }
    
    // Helper method to atomically match two requests
    fn atomic_match(&self, new_request: &QueuedRequest, matched_request_ref: &QueuedRequest) -> Result<MatchResult, BumpError> {
        // In this new implementation:
        // - new_request is always the request being added (has full channel)
        // - matched_request_ref is just a reference object with the ID of the matched request
        log::debug!("Starting atomic_match with new_request={} (type={:?}), matched_id={}", 
                  new_request.id, new_request.request_type, matched_request_ref.id);
        
        // Determine which is the send and which is the receive
        let (send_ref, receive_ref) = match (new_request.request_type, matched_request_ref.request_type) {
            (RequestType::Send, RequestType::Receive) => (new_request, matched_request_ref),
            (RequestType::Receive, RequestType::Send) => (matched_request_ref, new_request),
            _ => return Err(BumpError::ValidationError(
                format!("Cannot match two requests of the same type: {:?}", new_request.request_type)
            )),
        };
        
        // Create the match result outside the lock
        let now = time::OffsetDateTime::now_utc();
        let timestamp = (now.unix_timestamp_nanos() / 1_000_000) as i64;
        
        // Extract just the IDs we'll need
        let send_id = send_ref.id.clone();
        let receive_id = receive_ref.id.clone();
        
        // We need to identify which request is already in the map and which is being added
        let existing_in_map = if new_request.request_type == RequestType::Send {
            // New request is a Send, so we expect the Receive to be in the map
            receive_id.clone()
        } else {
            // New request is a Receive, so we expect the Send to be in the map
            send_id.clone()
        };
        
        // Create match results for both sides
        // Create match results based on the type of the new request
        let send_payload = if new_request.request_type == RequestType::Send {
            // If the new request is send, it has the payload
            new_request.payload.clone()
        } else {
            // If new request is receive, the payload would come from the request in the map
            None
        };
            
        let send_match_result = MatchResult {
            matched_with: receive_id.clone(),
            payload: send_payload.clone(),
            timestamp,
        };
        
        let receive_match_result = MatchResult {
            matched_with: send_id.clone(),
            payload: send_payload,
            timestamp,
        };
        
        // Take response channels and remove requests from the queue
        let (send_tx, receive_tx) = {
            // Acquire the write lock
            let mut requests = self.requests.write();
            
            // Debug - print the queue contents
            log::debug!("Queue contents during atomic_match:");
            for (id, req) in requests.iter() {
                log::debug!("  Queue item: id={}, type={:?}, key={:?}, channel={}", 
                         id, req.request_type, req.matching_data.custom_key, req.response_tx.is_some());
            }
            
            // First, verify that the existing request is in the map
            if !requests.contains_key(&existing_in_map) {
                log::error!("Expected request {} to be in map, but it was not found", existing_in_map);
                return Err(BumpError::NotFound(format!("Request {} not found in map", existing_in_map)));
            }
            
            // Get the request from the map
            let map_request = requests.remove(&existing_in_map)
                .ok_or_else(|| BumpError::NotFound(format!("Request {} not found in map", existing_in_map)))?;
            
            // For debugging
            log::debug!("Retrieved from map: request {} (type={:?}, has_channel={})",
                      map_request.id, map_request.request_type, map_request.response_tx.is_some());
            
            // Now we have:
            // - new_request: The request being added with a channel
            // - map_request: The request from the map which may also have a channel
            
            // Determine which is the send and which is the receive 
            let (mut send_request, mut receive_request) = if map_request.request_type == RequestType::Send {
                (map_request, new_request.clone())
            } else {
                (new_request.clone(), map_request)
            };
            
            log::debug!("Final mapped requests: send={} (channel={}), receive={} (channel={})",
                      send_request.id, send_request.response_tx.is_some(),
                      receive_request.id, receive_request.response_tx.is_some());
            
            // Mark both requests as matched
            send_request.state = RequestState::Matched;
            send_request.reserved_by = Some(receive_request.id.clone());
            
            receive_request.state = RequestState::Matched;
            receive_request.reserved_by = Some(send_request.id.clone());
            
            // Broadcast Matched events
            let _ = self.event_tx.send(RequestEvent {
                request: send_request.clone(),
                event_type: RequestEventType::Matched(receive_request.id.clone()),
            });
            
            let _ = self.event_tx.send(RequestEvent {
                request: receive_request.clone(),
                event_type: RequestEventType::Matched(send_request.id.clone()),
            });
            
            // Take the channels
            let send_tx = if send_request.response_tx.is_some() {
                log::debug!("Taking send request channel from {}", send_request.id);
                send_request.response_tx.take()
            } else {
                log::debug!("Send request {} has no channel", send_request.id);
                None
            };
            
            let receive_tx = if receive_request.response_tx.is_some() {
                log::debug!("Taking receive request channel from {}", receive_request.id);
                receive_request.response_tx.take()
            } else {
                log::debug!("Receive request {} has no channel", receive_request.id);
                None
            };
            
            // Broadcast Removed events
            let _ = self.event_tx.send(RequestEvent {
                request: send_request.clone(),
                event_type: RequestEventType::Removed,
            });
            
            let _ = self.event_tx.send(RequestEvent {
                request: receive_request.clone(),
                event_type: RequestEventType::Removed,
            });
            
            // Return the channels
            (send_tx, receive_tx)
        };
        
        // Send on the channels outside the lock
        if let Some(tx) = send_tx {
            log::info!("Notifying send request {} of match", send_id);
            if let Err(e) = tx.send(send_match_result.clone()) {
                log::error!("Failed to send match result to send request {}: {:?}", send_id, e);
            }
        }
        
        if let Some(tx) = receive_tx {
            log::info!("Notifying receive request {} of match", receive_id);
            if let Err(e) = tx.send(receive_match_result) {
                log::error!("Failed to send match result to receive request {}: {:?}", receive_id, e);
            }
        }
        
        Ok(send_match_result)
    }

    // Calculate match score between two requests
    pub fn calculate_match_score(&self, req1: &QueuedRequest, req2: &QueuedRequest) -> Option<i32> {
        // Don't match requests with same ID
        if req1.id == req2.id {
            log::debug!("Skipping self-match between {} and {}", req1.id, req2.id);
            return None;
        }
        
        // Don't match requests of the same type
        if req1.request_type == req2.request_type {
            log::debug!("Skipping match between {} and {}: both are {:?} requests", 
                      req1.id, req2.id, req1.request_type);
            return None;
        }
        
        let mut score = 0;
        let mut has_secondary_match = false;
        
        // Time proximity is scored based on how close the timestamps are
        let time_diff = (req1.matching_data.timestamp - req2.matching_data.timestamp).abs();
        
        // Get max time difference from the instance configuration
        let max_time_diff = self.max_time_diff_ms;
        
        log::info!("Time diff between {} and {}: {}ms (max allowed: {}ms)", 
                  req1.id, req2.id, time_diff, max_time_diff);
        
        // Log the timestamps for debugging
        log::info!("Timestamps: {} = {}, {} = {}", 
                 req1.id, req1.matching_data.timestamp,
                 req2.id, req2.matching_data.timestamp);
        
        if time_diff > max_time_diff {
            // Time difference too large
            log::debug!("Time difference too large ({}ms) between {} and {}", 
                      time_diff, req1.id, req2.id);
            return None;
        } else {
            // Calculate time proximity score (0-100)
            // 0 diff = 100 points, max_diff = 0 points, linear scale in between
            let time_score = 100 - ((time_diff as f64 / max_time_diff as f64) * 100.0) as i32;
            log::debug!("Time score for {} and {}: {}", req1.id, req2.id, time_score);
            // Time check passed
            score += time_score;
        }
        
        // Location match with distance-based scoring
        if let (Some(loc1), Some(loc2)) = (
            &req1.matching_data.location,
            &req2.matching_data.location
        ) {
            // Calculate distance between points (simplified here)
            let lat1 = loc1.lat.to_radians();
            let lat2 = loc2.lat.to_radians();
            let delta_lat = (loc2.lat - loc1.lat).to_radians();
            let delta_lon = (loc2.long - loc1.long).to_radians();
            
            // Haversine formula for distance calculation
            let a = (delta_lat / 2.0).sin().powi(2) + 
                   lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
            let c = 2.0 * a.sqrt().asin();
            
            // Calculate distance using Earth radius constant
            let distance = EARTH_RADIUS_METERS * c;
            
            // Max acceptable distance from the instance configuration
            let max_distance = self.max_distance_meters;
            
            log::debug!("Distance between {} and {}: {:.2}m (max allowed: {:.2}m)", 
                      req1.id, req2.id, distance, max_distance);
            
            if distance <= max_distance {
                // Calculate distance score (0-100)
                // 0 distance = 100 points, max_distance = 0 points
                let distance_score = 100 - ((distance / max_distance) * 100.0) as i32;
                log::debug!("Location match between {} and {}: distance={:.2}m, score={}", 
                          req1.id, req2.id, distance, distance_score);
                score += distance_score;
                has_secondary_match = true;
            } else {
                log::debug!("Location too far apart between {} and {}: {:.2}m", 
                          req1.id, req2.id, distance);
            }
        }
        
        // Custom key match - treated as an exact match requirement or boost
        // If both have custom keys, they must match
        if let (Some(k1), Some(k2)) = (
            &req1.matching_data.custom_key,
            &req2.matching_data.custom_key
        ) {
            log::debug!("Custom key check between {} ({}) and {} ({})", 
                      req1.id, k1, req2.id, k2);
            
            if k1 == k2 {
                // Custom key match
                log::debug!("Custom key match between {} and {}: {} == {}", 
                          req1.id, req2.id, k1, k2);
                has_secondary_match = true;
                // Custom key matching is a strong signal - give it a high score
                score += self.custom_key_match_bonus;
            } else {
                // If custom keys are provided but don't match, this is a strong
                // negative signal - we should never match these requests
                log::debug!("Custom keys don't match between {} and {}: {} != {}", 
                          req1.id, req2.id, k1, k2);
                return None;
            }
        } else if req1.matching_data.custom_key.is_some() || req2.matching_data.custom_key.is_some() {
            // If only one side has a custom key, no penalty but no boost either
            log::debug!("Only one request has a custom key between {} and {}", req1.id, req2.id);
        } else {
            // No custom keys specified
            log::debug!("No custom keys specified between {} and {}", req1.id, req2.id);
        }
        
        // Determine if we have a valid match based on combined criteria
        // We need either:
        // 1. A close time AND location match (high total score)
        // 2. A custom key match plus reasonable time proximity
        
        // If we have a custom key match, we should use a much lower threshold
        // because custom keys are explicit match identifiers
        let threshold = if has_secondary_match {
            if req1.matching_data.custom_key.is_some() && req2.matching_data.custom_key.is_some() {
                // If both have matching custom keys, use a very low threshold
                // Just need basic time proximity
                let custom_key_threshold = self.min_score_with_key / 2;  // Half the regular key threshold
                log::info!("Using low threshold (custom key match): {}", custom_key_threshold);
                custom_key_threshold  // Lower threshold for explicit key matches
            } else {
                log::info!("Using standard threshold with secondary match: {}", self.min_score_with_key);
                self.min_score_with_key
            }
        } else {
            // No custom key, rely on time+location
            log::info!("Using higher threshold (no custom key): {}", self.min_score_without_key);
            self.min_score_without_key
        };
        
        log::debug!("Total score for {} and {}: {} (threshold: {})", 
                  req1.id, req2.id, score, threshold);
        
        if score >= threshold {
            // Match found
            log::debug!("Match found between {} and {} with score {}", req1.id, req2.id, score);
            Some(score)
        } else {
            // Score below threshold
            log::debug!("Score below threshold between {} and {}: {} < {}", 
                      req1.id, req2.id, score, threshold);
            None
        }
    }
}

#[async_trait::async_trait]
impl RequestQueue for UnifiedQueue {
    /// Add a new request to the queue and immediately look for a match
    /// Returns:
    /// - Ok(Some(MatchResult)) if an immediate match was found
    /// - Ok(None) if no match was found and the request was added to the queue
    /// - Err(BumpError) if there was an error adding the request
    async fn add_request(&self, mut request: QueuedRequest) -> Result<Option<MatchResult>, BumpError> {
        // Ensure the request is in Active state
        request.state = RequestState::Active;
        request.reserved_by = None;
        
        // Check if the queue is full
        {
            let requests = self.requests.read();
            if requests.len() >= self.max_size {
                return Err(BumpError::QueueFull);
            }
        }
        
        // Look for an immediate match
        log::debug!("Searching for immediate match for request {} of type {:?}", request.id, request.request_type);
        
        // Dump queue contents for debugging
        {
            let requests_read = self.requests.read();
            log::debug!("Current queue contents ({} items):", requests_read.len());
            for (id, req) in requests_read.iter() {
                if let Some(key) = &req.matching_data.custom_key {
                    log::debug!("  Queue item: id={}, type={:?}, key={}, channel={}", 
                             id, req.request_type, key, req.response_tx.is_some());
                } else {
                    log::debug!("  Queue item: id={}, type={:?}, key=None, channel={}", 
                             id, req.request_type, req.response_tx.is_some());
                }
            }
        }
        
        if let Some(matched_id) = self.find_matching_request(&request) {
            // Found a match - try to atomically match them
            log::info!("Found immediate match for request {} with request {}", 
                    request.id, matched_id);
            
            // Get the matching request directly from the map in atomic_match
            // No need to check channel here since atomic_match will handle that
            
            // Create a temporary QueuedRequest with just the ID for reference
            let matched_req_ref = QueuedRequest {
                id: matched_id.clone(),
                matching_data: request.matching_data.clone(),  // Not actually used
                payload: None,
                expires_at: request.expires_at,     // Not actually used
                state: RequestState::Active,        // Not actually used
                reserved_by: None,
                request_type: match request.request_type {     // Opposite type
                    RequestType::Send => RequestType::Receive,
                    RequestType::Receive => RequestType::Send,
                },
                response_tx: None,                  // Not actually used
            };
            
            // Try the atomic match - the updated atomic_match handles retrieving from the map and sending to channels
            match self.atomic_match(&request, &matched_req_ref) {
                Ok(match_result) => {
                    // Match was successful
                    log::info!("Successfully matched request {} with request {}", 
                            request.id, matched_id);
                    
                    // Note: We don't need to send to the channel here anymore
                    // as atomic_match now handles notifying both channels
                    
                    return Ok(Some(match_result));
                },
                Err(e) => {
                    // Match failed - log and continue
                    log::warn!("Failed to match request {} with request {}: {:?}", 
                            request.id, matched_id, e);
                    
                    // Fall through to adding the request to the queue
                }
            }
        }
        
        // No immediate match or match failed - add the request to the queue
        {
            log::info!("Adding request {} to queue (type: {:?}, has_channel: {})", 
                     request.id, request.request_type, request.response_tx.is_some());
            
            // Create a clone for the event, before we move the original request
            let request_clone = request.clone();
            
            // Debug - dump channel status before adding
            log::debug!("Request {} before adding to queue: has_channel={}, type={:?}", 
                      request.id, request.response_tx.is_some(), request.request_type);
            
            // Insert the original request with channel intact, not a clone
            let mut requests = self.requests.write();
            requests.insert(request.id.clone(), request);
            
            // Debug - verify the request has been added correctly
            if let Some(stored_req) = requests.get(&request_clone.id) {
                log::debug!("Verified: request {} is in map: has_channel={}", 
                          request_clone.id, stored_req.response_tx.is_some());
            } else {
                log::error!("Failed to add request {} to map", request_clone.id);
            }
            
            // Broadcast Added event - use the clone for the event
            let _ = self.event_tx.send(RequestEvent {
                request: request_clone,
                event_type: RequestEventType::Added,
            });
        }
        
        Ok(None)
    }
    
    /// Remove a request from the queue
    async fn remove_request(&self, request_id: &str) -> Result<(), BumpError> {
        let mut requests = self.requests.write();
        
        if let Some(request) = requests.remove(request_id) {
            // Broadcast Removed event
            let _ = self.event_tx.send(RequestEvent {
                request: request.clone(),
                event_type: RequestEventType::Removed,
            });
            
            Ok(())
        } else {
            Err(BumpError::NotFound(format!("Request {} not found", request_id)))
        }
    }
    
    /// Subscribe to queue events
    fn subscribe(&self) -> broadcast::Receiver<RequestEvent> {
        self.event_tx.subscribe()
    }
    
    /// Clean up expired requests
    async fn cleanup_expired(&self) -> Result<(), BumpError> {
        let now = OffsetDateTime::now_utc();
        let mut expired_ids = Vec::new();
        
        // Find expired requests
        {
            let requests = self.requests.read();
            for (id, request) in requests.iter() {
                if request.expires_at <= now {
                    expired_ids.push((id.clone(), request.clone()));
                }
            }
        }
        
        // Remove expired requests
        {
            let mut requests = self.requests.write();
            for (id, _request) in &expired_ids {
                requests.remove(id);
            }
        }
        
        // Broadcast Expired events
        for (_, request) in expired_ids {
            let _ = self.event_tx.send(RequestEvent {
                request: request.clone(),
                event_type: RequestEventType::Expired,
            });
        }
        
        Ok(())
    }
    
    /// Get the current size of the queue
    fn size(&self) -> usize {
        let requests = self.requests.read();
        requests.len()
    }
    
    /// Get the maximum size of the queue
    fn capacity(&self) -> usize {
        self.max_size
    }
    
    /// Get the current size of the queue with error handling
    fn size_safe(&self) -> Result<usize, String> {
        Ok(self.size())
    }
    
    /// Get the maximum size of the queue with error handling
    fn capacity_safe(&self) -> Result<usize, String> {
        Ok(self.max_size)
    }
    
    /// Wait for a match for the given request
    ///
    /// DEPRECATED: This method is deprecated and should not be used with the new unified queue design.
    /// Applications should create a channel when adding the request and wait on that channel directly.
    async fn wait_for_match(&self, request_id: &str, ttl_ms: u64) -> Result<Option<MatchResult>, BumpError> {
        log::warn!("wait_for_match() is deprecated and may cause race conditions. Requests should include a channel when created.");
        
        // Create a channel for the match result
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        // Add the response channel to the request if it exists
        {
            let mut requests = self.requests.write();
            if let Some(request) = requests.get_mut(request_id) {
                request.response_tx = Some(tx);
            } else {
                // Request not found
                return Err(BumpError::NotFound(format!("Request {} not found", request_id)));
            }
        }
        
        // Create a timeout future
        let timeout_future = tokio::time::sleep(std::time::Duration::from_millis(ttl_ms));
        
        // Wait for either the match result or timeout
        tokio::select! {
            // Match found via oneshot channel
            result = rx => {
                match result {
                    Ok(match_result) => {
                        log::info!("Match found for request {}", request_id);
                        Ok(Some(match_result))
                    },
                    Err(_) => {
                        // Channel was dropped
                        log::warn!("Channel was dropped for request {}", request_id);
                        Err(BumpError::NotFound(format!("Request channel dropped for {}", request_id)))
                    }
                }
            },
            
            // Timeout reached
            _ = timeout_future => {
                log::info!("Timeout for request {}", request_id);
                
                // Remove the request from the queue
                let _ = self.remove_request(request_id).await;
                
                Err(BumpError::Timeout)
            },
        }
    }
}