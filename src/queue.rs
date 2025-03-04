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

/// Type of request (send, receive, or bump)
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum RequestType {
    /// Send request with payload
    Send,
    /// Receive request without payload
    Receive,
    /// Bump request (unified endpoint) that can have optional payload
    Bump,
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
        
        // For the unified /bump endpoint implementation, we need to handle requests differently
        // For the time being, we want to match bump requests with each other, so we'll
        // filter based on either:
        // 1. Request types are opposite (for backward compatibility)
        // 2. Special case for bump requests - both types are Send but have a custom_key match
        
        // Debug dump all potential match candidates
        log::debug!("Potential match candidates:");
        for (id, r) in requests.iter()
            .filter(|(_, r)| r.expires_at > now)
            .filter(|(_, r)| r.state == RequestState::Active)
            // We no longer filter by type here to keep ALL candidates
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
            // Special matching rule: either opposite type OR same type but not the same request
            .filter(|(id, r)| {
                let opposite_type = match request.request_type {
                    RequestType::Send => RequestType::Receive,
                    RequestType::Receive => RequestType::Send,
                    RequestType::Bump => RequestType::Bump, // Bump matches with other Bump requests
                };
                
                // Check for: 
                // 1. Traditional matching - opposite types
                // 2. Both are Send but have different IDs (for bump endpoint), and match on custom_key
                let traditional_match = r.request_type == opposite_type;
                let bump_match = r.request_type == request.request_type 
                               && id.as_str() != request.id
                               && r.matching_data.custom_key.is_some() 
                               && request.matching_data.custom_key.is_some()
                               && r.matching_data.custom_key == request.matching_data.custom_key;
                               
                traditional_match || bump_match
            })
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
            // Get additional information about the matched request
            if let Some(matched_req) = requests.get(id) {
                log::debug!("Found match with ID: {}, has_channel: {}, type: {:?}", 
                          id, matched_req.response_tx.is_some(), matched_req.request_type);
                
                // For Bump vs Bump matches, log extra details
                if request.request_type == RequestType::Bump && matched_req.request_type == RequestType::Bump {
                    log::info!("Found Bump-Bump match: {} (channel={}) with {} (channel={})",
                             request.id, request.response_tx.is_some(),
                             id, matched_req.response_tx.is_some());
                }
            } else {
                log::debug!("Found match with ID: {}, but it's no longer in the map", id);
            }
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
        
        // Special case for bump endpoint: if both are Send type and have custom_key,
        // we should allow the match between them
        let both_are_send_with_custom_key = new_request.request_type == RequestType::Send 
                                         && matched_request_ref.request_type == RequestType::Send
                                         && new_request.matching_data.custom_key.is_some()
                                         && matched_request_ref.matching_data.custom_key.is_some()
                                         && new_request.matching_data.custom_key == matched_request_ref.matching_data.custom_key;
        
        // Determine which is the send and which is the receive
        let (send_ref, receive_ref) = match (new_request.request_type, matched_request_ref.request_type) {
            // Traditional send/receive matching
            (RequestType::Send, RequestType::Receive) => (new_request, matched_request_ref),
            (RequestType::Receive, RequestType::Send) => (matched_request_ref, new_request),
            
            // Bump-to-Bump matching (symmetric)
            (RequestType::Bump, RequestType::Bump) => {
                // For Bump requests, we arbitrarily choose one as send and one as receive
                // This doesn't affect the actual behavior since both can have payloads
                (new_request, matched_request_ref)
            },
            
            // Legacy case for old bump endpoint using Send type
            _ if both_are_send_with_custom_key => {
                // Special case for old bump endpoint matching - both are Send type
                // Treat the one with the payload as Send and the other as Receive for processing
                if new_request.payload.is_some() {
                    (new_request, matched_request_ref)
                } else {
                    (matched_request_ref, new_request)
                }
            },
            
            // All other combinations are invalid
            _ => return Err(BumpError::ValidationError(
                format!("Cannot match incompatible request types: {:?} and {:?}", 
                        new_request.request_type, matched_request_ref.request_type)
            )),
        };
        
        // Create the match result outside the lock
        let now = time::OffsetDateTime::now_utc();
        let timestamp = (now.unix_timestamp_nanos() / 1_000_000) as i64;
        
        // Extract just the IDs we'll need
        let send_id = send_ref.id.clone();
        let receive_id = receive_ref.id.clone();
        
        // We need to identify which request is already in the map and which is being added
        // For Bump requests, use the ID directly since we know one must be in the map
        let existing_in_map = matched_request_ref.id.clone();
        
        // Create match results for both sides
        // For the bump endpoint, we need to exchange payloads in both directions
        let send_payload = send_ref.payload.clone();
        let receive_payload = receive_ref.payload.clone();
        
        // For bump requests, always exchange payloads in both directions
        // For traditional send/receive, only send -> receive gets payload
        let (send_receives, receive_receives) = if new_request.request_type == RequestType::Bump && matched_request_ref.request_type == RequestType::Bump {
            // Bump vs Bump: exchange payloads both ways
            // Use the original request references to ensure correct payload exchange
            (matched_request_ref.payload.clone(), new_request.payload.clone())
        } else if send_ref.request_type == RequestType::Send || receive_ref.request_type == RequestType::Receive {
            // Traditional send/receive: only receive gets payload
            (None, send_payload.clone())
        } else {
            // Other cases: no payload exchange
            (None, None)
        };
            
        let send_match_result = MatchResult {
            matched_with: receive_id.clone(),
            payload: send_receives,
            timestamp,
        };
        
        let receive_match_result = MatchResult {
            matched_with: send_id.clone(),
            payload: receive_receives,
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
            
            // For Bump requests, we need to maintain consistent send/receive roles
            // and carefully track which request has the channel to avoid dropping it
            // For Bump vs Bump matches, the add_request method has already added the 
            // new_request to the queue, but we've also removed map_request from the queue.
            // We need to ensure channels aren't dropped during this role assignment.
            
            // Create an exact clone of the new_request with its channel
            let new_request_with_channel = if new_request.id == existing_in_map {
                // If the new request is the one that came from the map, use the original
                map_request.clone()
            } else {
                // Otherwise use the new_request (which has its original channel)
                new_request.clone()
            };
            
            let (mut send_request, mut receive_request) = match (new_request.request_type, map_request.request_type) {
                (RequestType::Bump, RequestType::Bump) => {
                    // For Bump/Bump matches, preserve the channels and payloads carefully
                    // First request (map_request) has its channel preserved by removing from map above
                    // For the second request (new_request), we need its channel from the parameter
                    
                    // Log detailed debug info
                    log::debug!("Bump/Bump match: map_req.id={}, new_req.id={}, existing_in_map={}", 
                              map_request.id, new_request.id, existing_in_map);
                    log::debug!("Map request channel: {}, New request channel: {}", 
                               map_request.response_tx.is_some(), new_request_with_channel.response_tx.is_some());
                    
                    let map_req_has_channel = map_request.response_tx.is_some();
                    let new_req_has_channel = new_request_with_channel.response_tx.is_some();
                    
                    // For Bump vs Bump, both sides should get each other's payloads
                    // and preserve their own channels
                    let mut send = map_request;
                    let mut receive = new_request_with_channel;
                    
                    // CRITICAL: If we lost the channel for either request, take ownership of the other's channel
                    if !new_req_has_channel && map_req_has_channel {
                        log::warn!("Channel missing for new request - taking channel from map request");
                        receive.response_tx = send.response_tx.take();
                    } else if !map_req_has_channel && new_req_has_channel {
                        log::warn!("Channel missing for map request - taking channel from new request");
                        send.response_tx = receive.response_tx.take();
                    }
                    
                    (send, receive)
                },
                (RequestType::Send, _) | (_, RequestType::Receive) => {
                    // Send request is sender, other is receiver
                    if new_request.request_type == RequestType::Send || map_request.request_type == RequestType::Receive {
                        (new_request_with_channel, map_request)
                    } else {
                        (map_request, new_request_with_channel)
                    }
                },
                _ => {
                    // Receive request is receiver, other is sender 
                    if new_request.request_type == RequestType::Receive {
                        (map_request, new_request_with_channel)
                    } else {
                        (new_request_with_channel, map_request)
                    }
                }
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
        
        // For non-Bump types, don't match requests of the same type
        if req1.request_type == req2.request_type && req1.request_type != RequestType::Bump {
            log::debug!("Skipping match between {} and {}: both are {:?} requests", 
                      req1.id, req2.id, req1.request_type);
            return None;
        }
        
        // For Bump type, allow matching with other Bump requests
        // No special requirements - use the same criteria as send/receive
        
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
        // We MUST have:
        // 1. Time proximity (mandatory)
        // 2. At least one secondary factor (location or custom key)
        
        // If we don't have a secondary match, reject immediately
        if !has_secondary_match {
            log::debug!("No secondary match factor between {} and {}", req1.id, req2.id);
            return None;
        }
        
        // Set threshold based on matching factors
        let threshold = if req1.matching_data.custom_key.is_some() && req2.matching_data.custom_key.is_some() {
            // Custom key match - use lower threshold since it's an exact match
            let custom_key_threshold = self.min_score_with_key / 2;  // Half the regular key threshold
            log::info!("Using low threshold (custom key match): {}", custom_key_threshold);
            custom_key_threshold
        } else {
            // Location match only - use standard threshold
            log::info!("Using standard threshold with location match: {}", self.min_score_with_key);
            self.min_score_with_key
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
        
        // Store request type before moving request
        let request_id = request.id.clone();
        let request_type = request.request_type;
        
        // First check if we have an immediate match
        let request_clone = request.clone();
        let _matched_id = self.find_matching_request(&request); // Unused, we'll find match again after adding to queue
        
        // Add request to queue
        {
            log::info!("Adding request {} to queue (type: {:?}, has_channel: {})", 
                     request_id, request_type, request.response_tx.is_some());
            
            // Debug - dump channel status before adding
            log::debug!("Request {} before adding to queue: has_channel={}, type={:?}", 
                      request_id, request.response_tx.is_some(), request_type);
            
            // Insert the original request with channel intact
            let mut requests = self.requests.write();
            requests.insert(request_id.clone(), request);
            
            // Debug - verify the request has been added correctly
            if let Some(stored_req) = requests.get(&request_id) {
                log::debug!("Verified: request {} is in map: has_channel={}", 
                          request_id, stored_req.response_tx.is_some());
            }
        }

        // Process the match if we found one
        log::debug!("Searching for immediate match for request {} of type {:?}", request_id, request_type);
        
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
        
        if let Some(matched_id) = self.find_matching_request(&request_clone) {
            // Found a match - try to atomically match them
            log::info!("Found immediate match for request {} with request {}", 
                    request_id, matched_id);
            
            // Create a temporary QueuedRequest with just the ID for reference
            let matched_req_ref = QueuedRequest {
                id: matched_id.clone(),
                matching_data: request_clone.matching_data.clone(),  // Not actually used
                payload: None,
                expires_at: request_clone.expires_at,     // Not actually used
                state: RequestState::Active,        // Not actually used
                reserved_by: None,
                request_type: match request_type {     // Opposite type
                    RequestType::Send => RequestType::Receive,
                    RequestType::Receive => RequestType::Send,
                    RequestType::Bump => RequestType::Bump,  // Bump matches with itself
                },
                response_tx: None,                  // Not actually used
            };
            
            // Try the atomic match - the updated atomic_match handles retrieving from the map and sending to channels
            match self.atomic_match(&request_clone, &matched_req_ref) {
                Ok(match_result) => {
                    // Match was successful
                    log::info!("Successfully matched request {} with request {}", 
                            request_id, matched_id);
                    
                    // Note: We don't need to send to the channel here anymore
                    // as atomic_match now handles notifying both channels
                    
                    return Ok(Some(match_result));
                },
                Err(e) => {
                    // Match failed - log and continue
                    log::warn!("Failed to match request {} with request {}: {:?}", 
                            request_id, matched_id, e);
                }
            }
        }
        
        // Return None since no match was found
        // Broadcast Added event - use the clone for the event
        let _ = self.event_tx.send(RequestEvent {
            request: request_clone,
            event_type: RequestEventType::Added,
        });

        // Return None since no match was found
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