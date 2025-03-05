use tokio::sync::broadcast;
use time::OffsetDateTime;
use crate::models::MatchingData;
use crate::error::BumpError;

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
    /// For Bump requests, we use a broadcast channel so both sides can receive
    /// For traditional send/receive, we use a oneshot channel
    pub response_tx: Option<ResponseChannel>,
}

#[derive(Debug)]
pub enum ResponseChannel {
    /// Traditional oneshot channel for send/receive
    OneShot(tokio::sync::oneshot::Sender<(MatchResult, MatchResult)>),
    /// Broadcast channel for bump requests
    Broadcast(tokio::sync::broadcast::Sender<(MatchResult, MatchResult)>),
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
    async fn add_request(&self, request: QueuedRequest) -> Result<Option<(MatchResult, MatchResult)>, BumpError>;
    
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
    async fn wait_for_match(&self, request_id: &str, ttl_ms: u64) -> Result<Option<(MatchResult, MatchResult)>, BumpError>;
}