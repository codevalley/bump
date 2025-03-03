use std::future::Future;
use tokio::sync::broadcast;
use time::OffsetDateTime;
use uuid::Uuid;
use crate::models::{MatchingData, MatchResponse};
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
/// - Thread-safe using parking_lot::RwLock (more efficient than std::sync::RwLock)
/// - Real-time notifications using tokio::sync::broadcast
/// - Configurable maximum size to prevent memory exhaustion
/// - O(1) lookup and insertion
pub struct MemoryQueue {
    /// Thread-safe storage for pending requests
    requests: parking_lot::RwLock<std::collections::HashMap<String, QueuedRequest>>,
    /// Channel for broadcasting queue events to subscribers
    event_tx: broadcast::Sender<RequestEvent>,
    /// Maximum number of requests allowed in queue
    max_size: usize,
}

impl MemoryQueue {
    pub fn new(event_buffer: usize, max_size: usize) -> Self {
        let (tx, _) = broadcast::channel(event_buffer);
        Self {
            requests: parking_lot::RwLock::new(std::collections::HashMap::new()),
            event_tx: tx,
            max_size,
        }
    }
    
    fn calculate_match_score(req1: &QueuedRequest, req2: &QueuedRequest) -> Option<i32> {
        // Time proximity is mandatory
        let time_diff = (req1.matching_data.timestamp - req2.matching_data.timestamp).abs();
        if time_diff > 500 { // TODO: Make configurable
            return None;
        }
        
        let mut score = 0;
        let mut has_secondary_match = false;
        
        // Location match
        if let (Some(loc1), Some(loc2)) = (
            &req1.matching_data.location,
            &req2.matching_data.location
        ) {
            // TODO: Implement distance calculation
            has_secondary_match = true;
            score += 50;
        }
        
        // Custom key match
        if let (Some(k1), Some(k2)) = (
            &req1.matching_data.custom_key,
            &req2.matching_data.custom_key
        ) {
            if k1 == k2 {
                has_secondary_match = true;
                score += 100;
            }
        }
        
        if has_secondary_match {
            Some(score)
        } else {
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
        
        // Step 1: Check if queue is full BEFORE adding
        if requests.len() >= self.max_size {
            return Err(BumpError::QueueFull);
        }
        
        // Step 2: Add request to storage
        requests.insert(request.id.clone(), request.clone());
        
        // Step 3: Notify all subscribers about the new request
        let _ = self.event_tx.send(RequestEvent {
            request,
            event_type: RequestEventType::Added,
        });
        
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
        let requests = self.requests.read();
        let now = OffsetDateTime::now_utc();
        
        let best_match = requests.iter()
            .filter(|(_, r)| r.expires_at > now)
            .filter_map(|(_, r)| {
                Self::calculate_match_score(request, r)
                    .map(|score| (r.clone(), score))
            })
            .max_by_key(|(_, score)| *score)
            .map(|(r, _)| r);
            
        Ok(best_match)
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
