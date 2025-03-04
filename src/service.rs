use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use time::{OffsetDateTime, Duration};
use tokio::sync::broadcast;
use crate::models::{SendRequest, ReceiveRequest, BumpRequest, MatchResponse, MatchStatus, MatchingData, HealthStatus, QueueStats};
use crate::error::BumpError;
use crate::config::MatchingConfig;
use crate::queue::{UnifiedQueue, RequestEventType, RequestQueue, RequestType, QueuedRequest, RequestState, MatchResult};

#[derive(Clone)]
/// Service that handles matching of send and receive requests.
/// Uses a single unified queue for both send and receive requests.
pub struct MatchingService {
    /// Unified queue for both send and receive requests
    queue: Arc<UnifiedQueue>,
    /// Configuration for matching algorithm and service behavior
    config: MatchingConfig,
    /// Service start time for calculating uptime
    start_time: Arc<Instant>,
    /// Counter for successful matches
    matches_count: Arc<AtomicU64>,
    /// Counter for expired requests
    expired_count: Arc<AtomicU64>, 
}

impl MatchingService {
    /// Returns the current timestamp in milliseconds since epoch
    fn get_current_timestamp_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
    
    pub fn new(config: Option<MatchingConfig>) -> Self {
        let config = config.unwrap_or_default();
        let service = Self {
            queue: Arc::new(UnifiedQueue::new_with_config(
                100, 
                config.max_queue_size,
                config.max_time_diff_ms
            )), // Buffer size of 100 events
            config,
            start_time: Arc::new(Instant::now()),
            matches_count: Arc::new(AtomicU64::new(0)),
            expired_count: Arc::new(AtomicU64::new(0)),
        };
        
        // Start cleanup task
        service.start_cleanup_task();
        
        // Start metric collection for events
        service.start_metrics_collection();
        
        service
    }
    
    /// Sets up monitors for queue events to collect metrics
    fn start_metrics_collection(&self) {
        // Monitor queue events
        let events = self.queue.subscribe();
        let matches_count = self.matches_count.clone();
        let expired_count = self.expired_count.clone();
        
        tokio::spawn(async move {
            let mut rx = events;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        match event.event_type {
                            RequestEventType::Matched(_) => {
                                matches_count.fetch_add(1, Ordering::Relaxed);
                            },
                            RequestEventType::Expired => {
                                expired_count.fetch_add(1, Ordering::Relaxed);
                            },
                            _ => {}
                        }
                    },
                    Err(e) => {
                        if let broadcast::error::RecvError::Closed = e {
                            break;
                        }
                    }
                }
            }
        });
    }

    fn start_cleanup_task(&self) {
        let queue = self.queue.clone();
        let interval = std::time::Duration::from_millis(self.config.cleanup_interval_ms);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                let _ = queue.cleanup_expired().await;
            }
        });
    }

    #[allow(dead_code)]
    pub fn new_with_config(config: MatchingConfig) -> Self {
        Self::new(Some(config))
    }
    
    // Helper to create a request with a channel
    fn create_queued_request(
        id: String, 
        matching_data: MatchingData,
        payload: Option<String>,
        expires_at: OffsetDateTime,
        request_type: RequestType
    ) -> (QueuedRequest, tokio::sync::oneshot::Receiver<MatchResult>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request = QueuedRequest {
            id,
            matching_data,
            payload,
            expires_at,
            request_type,
            state: RequestState::Active,
            reserved_by: None,
            response_tx: Some(tx),
        };
        (request, rx)
    }

    /// Process a send request by adding it to the unified queue.
    /// The queue will automatically find matches or wait for new matching requests.
    ///
    /// The matching process follows these steps:
    /// 1. Validate the request has required matching criteria
    /// 2. Add the request to the unified queue
    /// 3. If no immediate match found, wait for a match or timeout
    ///
    /// # Returns
    /// - Ok(MatchResponse) if a match is found
    /// - Err(BumpError::Timeout) if no match found within TTL
    /// - Err(BumpError::QueueFull) if the queue is full
    pub async fn process_send(&self, request: SendRequest) -> Result<MatchResponse, BumpError> {
        // Step 1: Validate request has either location or custom key
        self.validate_matching_criteria(&request.matching_data)?;
        
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::milliseconds(request.ttl as i64);
        
        // Debug log the custom key and timestamp for this send request
        if let Some(key) = &request.matching_data.custom_key {
            log::info!("Processing send request with key: {}, timestamp: {}", 
                      key, request.matching_data.timestamp);
        }
        
        // Create a queued request with a channel
        // Create a copy of matching data with server-assigned timestamp
        let mut matching_data = request.matching_data.clone();
        matching_data.timestamp = Self::get_current_timestamp_ms();
        
        log::info!("Server-assigned timestamp for send request {}: {} (client timestamp was: {})",
                 request_id, matching_data.timestamp, request.matching_data.timestamp);
        
        let (queued_request, rx) = Self::create_queued_request(
            request_id.clone(),
            matching_data,
            Some(request.payload.clone()),
            expires_at,
            RequestType::Send,
        );
        
        // Add the request to the queue
        log::info!("Adding send request {} to queue", request_id);
        match self.queue.add_request(queued_request).await {
            Ok(Some(match_result)) => {
                // Immediate match found
                log::info!("Immediate match found for send request {}", request_id);
                
                return Ok(MatchResponse {
                    status: MatchStatus::Matched,
                    sender_id: Some(request_id),
                    receiver_id: Some(match_result.matched_with),
                    timestamp: match_result.timestamp,
                    payload: Some(request.payload),
                    message: None,
                });
            },
            Ok(None) => {
                // No immediate match found, wait for one using the rx channel
                log::info!("No immediate match found for send request {}, waiting...", request_id);
                
                // Wait for a match with timeout
                let ttl = std::time::Duration::from_millis(request.ttl as u64);
                match tokio::time::timeout(ttl, rx).await {
                    Ok(Ok(match_result)) => {
                        // We got a match result from the oneshot channel
                        log::info!("Match found for send request {}", request_id);
                        
                        return Ok(MatchResponse {
                            status: MatchStatus::Matched,
                            sender_id: Some(request_id),
                            receiver_id: Some(match_result.matched_with),
                            timestamp: match_result.timestamp,
                            payload: Some(request.payload),
                            message: None,
                        });
                    },
                    Ok(Err(_recv_error)) => {
                        // The channel was dropped, maybe the request was removed
                        log::warn!("Channel was dropped for send request {}", request_id);
                        return Err(BumpError::NotFound("Request channel dropped".to_string()));
                    },
                    Err(_timeout) => {
                        // Timed out waiting for a match
                        log::info!("No match found for send request {} (timeout)", request_id);
                        let _ = self.queue.remove_request(&request_id).await;
                        return Err(BumpError::Timeout);
                    }
                }
            },
            Err(e) => {
                log::warn!("Failed to add send request {}: {:?}", request_id, e);
                return Err(e);
            }
        }
    }
    
    pub async fn process_receive(&self, request: ReceiveRequest) -> Result<MatchResponse, BumpError> {
        // Validate request has either location or custom key
        self.validate_matching_criteria(&request.matching_data)?;
        
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::milliseconds(request.ttl as i64);
        
        // Debug log the custom key and timestamp for this receive request
        if let Some(key) = &request.matching_data.custom_key {
            log::info!("Processing receive request with key: {}, timestamp: {}", 
                      key, request.matching_data.timestamp);
        }
        
        // Create a copy of matching data with server-assigned timestamp
        let mut matching_data = request.matching_data.clone();
        matching_data.timestamp = Self::get_current_timestamp_ms();
        
        log::info!("Server-assigned timestamp for receive request {}: {} (client timestamp was: {})",
                 request_id, matching_data.timestamp, request.matching_data.timestamp);
                 
        // Create a queued request with a channel
        let (queued_request, rx) = Self::create_queued_request(
            request_id.clone(),
            matching_data,
            None, // Receivers don't have payload
            expires_at,
            RequestType::Receive,
        );
        
        // Add the request to the unified queue
        log::info!("Adding receive request {} to queue", request_id);
        match self.queue.add_request(queued_request).await {
            Ok(Some(match_result)) => {
                // Immediate match found
                log::info!("Immediate match found for receive request {}", request_id);
                
                return Ok(MatchResponse {
                    status: MatchStatus::Matched,
                    sender_id: Some(match_result.matched_with),
                    receiver_id: Some(request_id),
                    timestamp: match_result.timestamp,
                    payload: match_result.payload,
                    message: None,
                });
            },
            Ok(None) => {
                // No immediate match found, wait for one using the rx channel
                log::info!("No immediate match found for receive request {}, waiting...", request_id);
                
                // Wait for a match with timeout
                let ttl = std::time::Duration::from_millis(request.ttl as u64);
                match tokio::time::timeout(ttl, rx).await {
                    Ok(Ok(match_result)) => {
                        // We got a match result from the oneshot channel
                        log::info!("Match found for receive request {}", request_id);
                        
                        return Ok(MatchResponse {
                            status: MatchStatus::Matched,
                            sender_id: Some(match_result.matched_with),
                            receiver_id: Some(request_id),
                            timestamp: match_result.timestamp,
                            payload: match_result.payload,
                            message: None,
                        });
                    },
                    Ok(Err(_recv_error)) => {
                        // The channel was dropped, maybe the request was removed
                        log::warn!("Channel was dropped for receive request {}", request_id);
                        return Err(BumpError::NotFound("Request channel dropped".to_string()));
                    },
                    Err(_timeout) => {
                        // Timed out waiting for a match
                        log::info!("No match found for receive request {} (timeout)", request_id);
                        let _ = self.queue.remove_request(&request_id).await;
                        return Err(BumpError::Timeout);
                    }
                }
            },
            Err(e) => {
                log::warn!("Failed to add receive request {}: {:?}", request_id, e);
                return Err(e);
            }
        }
    }
    
    /// Process a unified bump request that can both send and receive data.
    /// Handles matching with other bump requests using a symmetric matching process.
    ///
    /// Matching criteria are identical to send/receive:
    /// 1. Time proximity (within configured threshold)
    /// 2. Location proximity (if provided)
    /// 3. Custom key match (if provided)
    ///
    /// When matched:
    /// - Both sides can optionally provide payloads
    /// - Each side receives the other's payload if provided
    /// - The matching is symmetric (no sender/receiver distinction)
    ///
    /// # Returns
    /// - Ok(MatchResponse) if a match is found
    /// - Err(BumpError::ValidationError) if no matching criteria provided
    /// - Err(BumpError::Timeout) if no match found within TTL
    /// - Err(BumpError::QueueFull) if the queue is full
    pub async fn process_bump(&self, request: BumpRequest) -> Result<MatchResponse, BumpError> {
        // Step 1: Validate request has either location or custom key
        self.validate_matching_criteria(&request.matching_data)?;
        
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::milliseconds(request.ttl as i64);
        
        // Debug log the custom key and timestamp for this bump request
        if let Some(key) = &request.matching_data.custom_key {
            log::info!("Processing bump request with key: {}, timestamp: {}", 
                      key, request.matching_data.timestamp);
        }
        
        // Create a copy of matching data with server-assigned timestamp
        let mut matching_data = request.matching_data.clone();
        matching_data.timestamp = Self::get_current_timestamp_ms();
        
        log::info!("Server-assigned timestamp for bump request {}: {} (client timestamp was: {})",
                 request_id, matching_data.timestamp, request.matching_data.timestamp);
        
        // Create a queued request with a channel
        let (queued_request, rx) = Self::create_queued_request(
            request_id.clone(),
            matching_data,
            request.payload.clone(),  // May be None or Some
            expires_at,
            RequestType::Bump, // Use the new Bump type for unified endpoint
        );
        
        // Add the request to the queue
        log::info!("Adding bump request {} to queue", request_id);
        match self.queue.add_request(queued_request).await {
            Ok(Some(match_result)) => {
                // Immediate match found
                log::info!("Immediate match found for bump request {}", request_id);
                
                return Ok(MatchResponse {
                    status: MatchStatus::Matched,
                    sender_id: Some(request_id),
                    receiver_id: Some(match_result.matched_with),
                    timestamp: match_result.timestamp,
                    payload: match_result.payload,
                    message: None,
                });
            },
            Ok(None) => {
                // No immediate match found, wait for one using the rx channel
                log::info!("No immediate match found for bump request {}, waiting...", request_id);
                
                // Wait for a match with timeout
                let ttl = std::time::Duration::from_millis(request.ttl as u64);
                match tokio::time::timeout(ttl, rx).await {
                    Ok(Ok(match_result)) => {
                        // We got a match result from the oneshot channel
                        log::info!("Match found for bump request {}", request_id);
                        
                        return Ok(MatchResponse {
                            status: MatchStatus::Matched,
                            sender_id: Some(request_id),
                            receiver_id: Some(match_result.matched_with),
                            timestamp: match_result.timestamp,
                            payload: match_result.payload,
                            message: None,
                        });
                    },
                    Ok(Err(_recv_error)) => {
                        // The channel was dropped, maybe the request was removed
                        log::warn!("Channel was dropped for bump request {}", request_id);
                        return Err(BumpError::NotFound("Request channel dropped".to_string()));
                    },
                    Err(_timeout) => {
                        // Timed out waiting for a match
                        log::info!("No match found for bump request {} (timeout)", request_id);
                        let _ = self.queue.remove_request(&request_id).await;
                        return Err(BumpError::Timeout);
                    }
                }
            },
            Err(e) => {
                log::warn!("Failed to add bump request {}: {:?}", request_id, e);
                return Err(e);
            }
        }
    }
    
    fn validate_matching_criteria(&self, data: &MatchingData) -> Result<(), BumpError> {
        // Must have either location or custom key
        if data.location.is_none() && data.custom_key.is_none() {
            return Err(BumpError::ValidationError(
                "Request must include either location or custom key".to_string()
            ));
        }

        // Validate location if present
        if let Some(location) = &data.location {
            if !(-90.0..=90.0).contains(&location.lat) || !(-180.0..=180.0).contains(&location.long) {
                return Err(BumpError::ValidationError(
                    "Invalid location coordinates".to_string()
                ));
            }
        }

        Ok(())
    }
    
    /// Get health status information for the service
    pub fn get_health_status(&self) -> Result<HealthStatus, String> {
        // Current version from Cargo.toml
        let version = env!("CARGO_PKG_VERSION");
        
        // Calculate uptime - this should always work
        let uptime_seconds = self.start_time.elapsed().as_secs();
        
        // Get queue size - could potentially fail if locks are poisoned
        let queue_size = match self.queue.size_safe() {
            Ok(size) => size,
            Err(e) => return Err(format!("Failed to get queue size: {:?}", e)),
        };
        
        // Get match and expired counts - atomic operations should be safe
        let matches_count = self.matches_count.load(Ordering::Relaxed);
        let expired_count = self.expired_count.load(Ordering::Relaxed);
        
        // Calculate match rate (matches per second)
        let match_rate = if uptime_seconds > 0 {
            matches_count as f64 / uptime_seconds as f64
        } else {
            0.0
        };
        
        // Additional system metrics
        let mut metrics = HashMap::new();
        
        // Safely get capacity
        match self.queue.capacity_safe() {
            Ok(capacity) => { metrics.insert("queue_capacity".to_string(), capacity as u64); },
            Err(_) => { metrics.insert("queue_capacity".to_string(), 0); },
        }
        
        // These should always be available from the config
        metrics.insert("cleanup_interval_ms".to_string(), self.config.cleanup_interval_ms);
        metrics.insert("max_time_diff_ms".to_string(), self.config.max_time_diff_ms as u64);
        metrics.insert("max_distance_meters".to_string(), self.config.max_distance_meters as u64);
        
        // For compatibility, report the unified queue size as both send and receive
        let send_queue_size = queue_size;
        let receive_queue_size = 0; // Not used anymore
        
        Ok(HealthStatus {
            status: "ok".to_string(),
            version: version.to_string(),
            uptime_seconds,
            metrics,
            queue_stats: QueueStats {
                send_queue_size,
                receive_queue_size,
                matches_count,
                expired_count,
                match_rate,
            },
        })
    }
}