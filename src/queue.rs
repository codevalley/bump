use tokio::sync::broadcast;
use time::OffsetDateTime;
use crate::models::MatchingData;
use crate::error::BumpError;

/// A request stored in the queue awaiting a match.
/// Each request has a unique ID, matching criteria, optional payload,
/// and an expiration time.
#[derive(Clone, Debug)]
pub struct QueuedRequest {
    /// Unique identifier for this request
    pub id: String,
    /// Criteria used for matching (timestamp, location, custom key)
    pub matching_data: MatchingData,
    /// Optional payload (present for send requests, None for receive)
    pub payload: Option<String>,
    /// When this request should be removed from queue
    pub expires_at: OffsetDateTime,
}

/// Event emitted when the queue state changes.
/// These events are broadcast to all subscribers and used for real-time matching.
#[derive(Clone, Debug)]
pub struct RequestEvent {
    /// The request involved in this event
    pub request: QueuedRequest,
    /// Type of event (Added, Matched, Expired, Removed)
    pub event_type: RequestEventType,
}

#[derive(Clone, Debug)]
pub enum RequestEventType {
    Added,
    #[allow(dead_code)]
    Matched(String), // ID of the matching request
    Expired,
    Removed,
}

/// Trait defining the interface for a request queue.
/// This abstraction allows for different queue implementations (memory, Redis, etc.)
/// while maintaining the same matching behavior.
///
/// All implementations must be thread-safe and support async operations.
#[async_trait::async_trait]
pub trait RequestQueue: Send + Sync + 'static {
    /// Add a new request to the queue
    async fn add_request(&self, request: QueuedRequest) -> Result<(), BumpError>;
    
    /// Remove a request from the queue
    async fn remove_request(&self, request_id: &str) -> Result<(), BumpError>;
    
    /// Find a matching request based on the given criteria
    async fn find_match(&self, request: &QueuedRequest) -> Result<Option<QueuedRequest>, BumpError>;
    
    /// Subscribe to queue events
    fn subscribe(&self) -> broadcast::Receiver<RequestEvent>;
    
    /// Clean up expired requests
    async fn cleanup_expired(&self) -> Result<(), BumpError>;
}

/// In-memory implementation of RequestQueue using channels for notifications.
/// Uses a HashMap protected by RwLock for storing requests and a broadcast channel
/// for real-time event notifications.
///
/// This implementation has the following characteristics:
/// - Thread-safe using Arc<RwLock<HashMap>> to share state between clones
/// - Real-time notifications using tokio::sync::broadcast
/// - Configurable maximum size to prevent memory exhaustion
/// - O(1) lookup and insertion
pub struct MemoryQueue {
    /// Thread-safe shared storage for pending requests
    requests: std::sync::Arc<parking_lot::RwLock<std::collections::HashMap<String, QueuedRequest>>>,
    /// Channel for broadcasting queue events to subscribers
    event_tx: broadcast::Sender<RequestEvent>,
    /// Maximum number of requests allowed in queue
    max_size: usize,
}

impl Clone for MemoryQueue {
    fn clone(&self) -> Self {
        // Clone just shares the Arc references, ensuring all clones
        // operate on the same underlying data
        Self {
            requests: self.requests.clone(),
            event_tx: self.event_tx.clone(),
            max_size: self.max_size,
        }
    }
}

impl MemoryQueue {
    pub fn new(event_buffer: usize, max_size: usize) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        Self {
            requests: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            event_tx: tx,
            max_size,
        }
    }
    
    // Configuration methods - these would normally fetch from a shared config
    // but are hardcoded here for testing. In production, these would use the
    // MatchingConfig struct or a similar configuration source.
    
    // Maximum time difference between requests to be considered for matching (ms)
    fn get_config_max_time_diff_ms() -> i64 {
        500 // Default: 500ms
    }
    
    // Maximum distance between requests to be considered for matching (meters)
    fn get_config_max_distance_meters() -> f64 {
        5.0 // Default: 5 meters
    }
    
    // Earth radius for geospatial calculations (meters)
    fn get_config_earth_radius_meters() -> f64 {
        6_371_000.0 // Standard Earth radius
    }
    
    // Minimum score required for a match when no custom key is present
    fn get_config_min_score_without_key() -> i32 {
        150 // Default: need good time+location scores
    }
    
    // Minimum score required for a match when a custom key is present
    fn get_config_min_score_with_key() -> i32 {
        100 // Default: lower threshold with matching key
    }
    
    // Threshold for considering a match to have a custom key boost
    fn get_config_key_match_threshold() -> i32 {
        250 // Default: threshold for considering a key-boosted match
    }
    
    pub fn calculate_match_score(req1: &QueuedRequest, req2: &QueuedRequest) -> Option<i32> {
        // Debug matching
        println!("Matching:\n  req1: id={}, payload={:?}, custom_key={:?}\n  req2: id={}, payload={:?}, custom_key={:?}",
            req1.id, req1.payload, req1.matching_data.custom_key,
            req2.id, req2.payload, req2.matching_data.custom_key);
            
        // Don't match requests with same ID
        if req1.id == req2.id {
            println!("  No match - Same ID");
            return None;
        }
        
        // Don't match if both have payloads (both are send requests)
        // or neither have payloads (both are receive requests)
        match (req1.payload.is_some(), req2.payload.is_some()) {
            (true, true) => {
                println!("  No match - Both have payloads (both send requests)");
                return None;
            },
            (false, false) => {
                println!("  No match - Neither have payloads (both receive requests)");
                return None;
            },
            _ => {
                println!("  Payload check passed (one send, one receive)");
            }
        }
        
        let mut score = 0;
        let mut has_secondary_match = false;
        
        // Time proximity is scored based on how close the timestamps are
        let time_diff = (req1.matching_data.timestamp - req2.matching_data.timestamp).abs();
        
        // Get max time difference from config
        // This is a placeholder - in a real implementation, we would get this from a shared config
        // For now, we hardcode to 500ms for test compatibility
        let max_time_diff = Self::get_config_max_time_diff_ms();
        
        if time_diff > max_time_diff {
            println!("  No match - Time difference too large: {}ms (max: {}ms)", time_diff, max_time_diff);
            return None;
        } else {
            // Calculate time proximity score (0-100)
            // 0 diff = 100 points, max_diff = 0 points, linear scale in between
            let time_score = 100 - ((time_diff as f64 / max_time_diff as f64) * 100.0) as i32;
            println!("  Time check passed: diff={}ms, score={}", time_diff, time_score);
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
            
            // Earth radius in meters from config
            let earth_radius = Self::get_config_earth_radius_meters();
            let distance = earth_radius * c;
            
            // Max acceptable distance from config
            let max_distance = Self::get_config_max_distance_meters();
            
            if distance <= max_distance {
                // Calculate distance score (0-100)
                // 0 distance = 100 points, max_distance = 0 points
                let distance_score = 100 - ((distance / max_distance) * 100.0) as i32;
                println!("  Location match: distance={:.2}m, score={}", distance, distance_score);
                score += distance_score;
                has_secondary_match = true;
            } else {
                println!("  Location too far: {:.2}m (max: {:.2}m)", distance, max_distance);
            }
        }
        
        // Custom key match - treated as an exact match requirement or boost
        // If both have custom keys, they must match
        if let (Some(k1), Some(k2)) = (
            &req1.matching_data.custom_key,
            &req2.matching_data.custom_key
        ) {
            if k1 == k2 {
                println!("  Custom key match: {}", k1);
                has_secondary_match = true;
                // Custom key matching is a strong signal - give it a high score
                score += 200;
            } else {
                // If custom keys are provided but don't match, this is a strong
                // negative signal - we should never match these requests
                println!("  Custom keys don't match: {} vs {}", k1, k2);
                return None;
            }
        } else if req1.matching_data.custom_key.is_some() || req2.matching_data.custom_key.is_some() {
            // If only one side has a custom key, no penalty but no boost either
            println!("  Only one request has a custom key");
        } else {
            println!("  No custom keys specified");
        }
        
        // Determine if we have a valid match based on combined criteria
        // We need either:
        // 1. A close time AND location match (high total score)
        // 2. A custom key match plus reasonable time proximity
        let min_score_without_key = Self::get_config_min_score_without_key(); // Need good time+location scores
        let min_score_with_key = Self::get_config_min_score_with_key();       // Lower threshold with matching key
        
        let threshold = if has_secondary_match && score > Self::get_config_key_match_threshold() {
            // Custom key match present 
            min_score_with_key
        } else {
            // No custom key, rely on time+location
            min_score_without_key
        };
        
        if score >= threshold {
            println!("  Match found with score {} (threshold: {})", score, threshold);
            Some(score)
        } else {
            println!("  Score {} below threshold {}", score, threshold);
            None
        }
    }
}

#[async_trait::async_trait]
impl RequestQueue for MemoryQueue {
    /// Add a new request to the queue.
    /// This operation will fail if the queue is at capacity (len >= max_size).
    ///
    /// The process is:
    /// 1. Acquire write lock on requests
    /// 2. Check if queue is full
    /// 3. Add request to storage
    /// 4. Broadcast Added event to all subscribers
    ///
    /// # Returns
    /// - Ok(()) if request was added successfully
    /// - Err(BumpError::QueueFull) if queue is at capacity
    async fn add_request(&self, request: QueuedRequest) -> Result<(), BumpError> {
        let mut requests = self.requests.write();
        println!("Adding request: id={} to queue (current size: {})", request.id, requests.len());
        
        // Step 1: Check if queue is full BEFORE adding
        if requests.len() >= self.max_size {
            println!("Queue is full, rejecting request");
            return Err(BumpError::QueueFull);
        }
        
        // Step 2: Add request to storage
        requests.insert(request.id.clone(), request.clone());
        println!("Added request. New queue size: {}", requests.len());
        
        // Step 3: Notify all subscribers about the new request
        match self.event_tx.send(RequestEvent {
            request: request.clone(),
            event_type: RequestEventType::Added,
        }) {
            Ok(n) => println!("Notified {} subscribers about new request", n),
            Err(e) => println!("Failed to notify subscribers: {}", e),
        }
        
        Ok(())
    }
    
    async fn remove_request(&self, request_id: &str) -> Result<(), BumpError> {
        let mut requests = self.requests.write();
        if let Some(request) = requests.remove(request_id) {
            // Notify subscribers
            let _ = self.event_tx.send(RequestEvent {
                request,
                event_type: RequestEventType::Removed,
            });
        }
        Ok(())
    }
    
    async fn find_match(&self, request: &QueuedRequest) -> Result<Option<QueuedRequest>, BumpError> {
        let mut requests = self.requests.write();
        let now = OffsetDateTime::now_utc();
        
        println!("Finding match for request: id={}", request.id);
        println!("Current queue size: {}", requests.len());
        
        // Debug print all requests in the queue
        for (id, r) in requests.iter() {
            println!("Queue entry: id={}, payload={:?}, timestamp={}, expires={}, expired={}",
                id, r.payload, r.matching_data.timestamp, r.expires_at, r.expires_at <= now);
        }
        
        let best_match = requests.iter()
            .filter(|(_, r)| r.expires_at > now)
            .filter_map(|(id, r)| {
                Self::calculate_match_score(request, r)
                    .map(|score| (id.clone(), r.clone(), score))
            })
            .max_by_key(|(_, _, score)| *score);
            
        if let Some((id, matched_req, score)) = best_match {
            println!("Found match: id={}, score={}", id, score);
            // Remove the matched request from the queue
            requests.remove(&id);
            Ok(Some(matched_req))
        } else {
            println!("No match found");
            Ok(None)
        }
    }
    
    fn subscribe(&self) -> broadcast::Receiver<RequestEvent> {
        self.event_tx.subscribe()
    }
    
    async fn cleanup_expired(&self) -> Result<(), BumpError> {
        let mut requests = self.requests.write();
        let now = OffsetDateTime::now_utc();
        
        let expired: Vec<_> = requests.iter()
            .filter(|(_, r)| r.expires_at <= now)
            .map(|(id, _)| id.clone())
            .collect();
            
        for id in expired {
            if let Some(request) = requests.remove(&id) {
                let _ = self.event_tx.send(RequestEvent {
                    request,
                    event_type: RequestEventType::Expired,
                });
            }
        }
        
        Ok(())
    }
}
