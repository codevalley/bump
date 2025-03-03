use std::sync::Arc;
use time::{OffsetDateTime, Duration};
use tokio::sync::broadcast;
use tokio::time::timeout;
use crate::models::{SendRequest, ReceiveRequest, MatchResponse, MatchStatus, Location, MatchingData};
use crate::error::BumpError;
use crate::config::MatchingConfig;
use crate::queue::{QueuedRequest, MemoryQueue, RequestEvent, RequestEventType, RequestQueue};

#[derive(Clone)]
/// Service that handles matching of send and receive requests.
/// Uses two separate queues for send and receive requests, with event-based notifications
/// for real-time matching.
pub struct MatchingService {
    /// Queue for send requests, with configurable size limit
    send_queue: Arc<MemoryQueue>,
    /// Queue for receive requests, with configurable size limit
    receive_queue: Arc<MemoryQueue>,
    /// Configuration for matching algorithm and service behavior
    config: MatchingConfig,
}

impl MatchingService {
    pub fn new(config: Option<MatchingConfig>) -> Self {
        let config = config.unwrap_or_default();
        let service = Self {
            send_queue: Arc::new(MemoryQueue::new(100, config.max_queue_size)), // Buffer size of 100 events
            receive_queue: Arc::new(MemoryQueue::new(100, config.max_queue_size)),
            config,
        };
        
        // Start cleanup task
        service.start_cleanup_task();
        
        service
    }

    fn start_cleanup_task(&self) {
        let send_queue = self.send_queue.clone();
        let receive_queue = self.receive_queue.clone();
        let interval = std::time::Duration::from_millis(self.config.cleanup_interval_ms);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                let _ = send_queue.cleanup_expired().await;
                let _ = receive_queue.cleanup_expired().await;
            }
        });
    }

    #[allow(dead_code)]
    pub fn new_with_config(config: MatchingConfig) -> Self {
        Self::new(Some(config))
    }

    /// Process a send request by attempting to match it with an existing receive request
    /// or waiting for a new matching receive request.
    ///
    /// The matching process follows these steps:
    /// 1. Validate the request has required matching criteria
    /// 2. Check for existing matching receive requests (even if receive queue is full)
    /// 3. If no match found, try to add to send queue (may fail if queue is full)
    /// 4. Wait for matching receive request or timeout
    ///
    /// # Returns
    /// - Ok(MatchResponse) if a match is found
    /// - Err(BumpError::Timeout) if no match found within TTL
    /// - Err(BumpError::QueueFull) if no immediate match and queue is full
    pub async fn process_send(&self, request: SendRequest) -> Result<MatchResponse, BumpError> {
        // Step 1: Validate request has either location or custom key
        self.validate_matching_criteria(&request.matching_data)?;
        
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::milliseconds(request.ttl as i64);
        
        // Create queued request with payload
        let queued_request = QueuedRequest {
            id: request_id.clone(),
            matching_data: request.matching_data.clone(),
            payload: Some(request.payload.clone()),
            expires_at,
        };
        
        // Step 2: Subscribe to receive queue events BEFORE checking for matches
        // This ensures we don't miss any matches that occur while we're checking
        let receive_events = self.receive_queue.subscribe();
        
        // Check for existing matches in receive queue
        // We do this before checking queue size to maximize matching success
        if let Some(matched_request) = self.receive_queue.find_match(&queued_request).await? {
            return Ok(MatchResponse {
                status: MatchStatus::Matched,
                sender_id: Some(request_id),
                receiver_id: Some(matched_request.id),
                timestamp: (now.unix_timestamp_nanos() / 1_000_000) as i64,
                payload: Some(request.payload),
                message: None,
            });
        }
        
        // Step 3: No immediate match found, try to add to queue
        // This may fail with QueueFull error if queue is at capacity
        self.send_queue.add_request(queued_request.clone()).await?;
        
        // Step 4: Wait for matching event or timeout
        let ttl = std::time::Duration::from_millis(request.ttl as u64);
        match timeout(ttl, self.wait_for_match(receive_events, &queued_request)).await {
            Ok(result) => result,
            Err(_) => {
                // Timeout occurred, cleanup our request
                self.send_queue.remove_request(&request_id).await?;
                Err(BumpError::Timeout)
            }
        }
    }
    
    /// Wait for a matching request event from the receive queue.
    /// This is called after we've added our request to the send queue and are waiting
    /// for a matching receive request to arrive.
    ///
    /// # Arguments
    /// * `events` - Event receiver for the receive queue
    /// * `our_request` - Our send request that we're trying to match
    ///
    /// # Returns
    /// - Ok(MatchResponse) if a match is found
    /// - Err(BumpError::Timeout) if no match found within TTL
    async fn wait_for_match(
        &self,
        mut events: broadcast::Receiver<RequestEvent>,
        our_request: &QueuedRequest,
    ) -> Result<MatchResponse, BumpError> {
        while let Ok(event) = events.recv().await {
            match event.event_type {
                RequestEventType::Added => {
                    // Check if this new request matches ours
                    if let Some(_) = MemoryQueue::calculate_match_score(our_request, &event.request) {
                        return Ok(MatchResponse {
                            status: MatchStatus::Matched,
                            sender_id: Some(our_request.id.clone()),
                            receiver_id: Some(event.request.id),
                            timestamp: (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
                            payload: our_request.payload.clone(),
                            message: None,
                        });
                    }
                },
                _ => continue,
            }
        }
        
        Err(BumpError::Timeout)
    }



    pub async fn process_receive(&self, request: ReceiveRequest) -> Result<MatchResponse, BumpError> {
        // Validate request has either location or custom key
        self.validate_matching_criteria(&request.matching_data)?;
        
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + Duration::milliseconds(request.ttl as i64);
        
        // Create queued request
        let queued_request = QueuedRequest {
            id: request_id.clone(),
            matching_data: request.matching_data.clone(),
            payload: None, // Receivers don't have payload
            expires_at,
        };
        
        // Subscribe to send queue events before adding our request
        let send_events = self.send_queue.subscribe();
        
        // First check for existing matches
        if let Some(matched_request) = self.send_queue.find_match(&queued_request).await? {
            // Found a match! Remove the matched request and return its payload
            self.send_queue.remove_request(&matched_request.id).await?;
            
            return Ok(MatchResponse {
                status: MatchStatus::Matched,
                sender_id: Some(matched_request.id),
                receiver_id: Some(request_id),
                timestamp: (now.unix_timestamp_nanos() / 1_000_000) as i64,
                payload: matched_request.payload,
                message: None,
            });
        }
        
        // Add request to queue
        self.receive_queue.add_request(queued_request.clone()).await?;
        
        // Wait for matching event or timeout
        let ttl = std::time::Duration::from_millis(request.ttl as u64);
        match timeout(ttl, self.wait_for_send_match(send_events, &queued_request)).await {
            Ok(result) => result,
            Err(_) => {
                // Timeout occurred
                self.receive_queue.remove_request(&request_id).await?;
                Err(BumpError::Timeout)
            }
        }
    }
    
    async fn wait_for_send_match(
        &self,
        mut events: broadcast::Receiver<RequestEvent>,
        our_request: &QueuedRequest,
    ) -> Result<MatchResponse, BumpError> {
        while let Ok(event) = events.recv().await {
            match event.event_type {
                RequestEventType::Added => {
                    // Check if this new request matches ours
                    if let Some(_) = MemoryQueue::calculate_match_score(our_request, &event.request) {
                        // Found a match! Remove the matched request
                        self.send_queue.remove_request(&event.request.id).await?;
                        self.receive_queue.remove_request(&our_request.id).await?;
                        
                        return Ok(MatchResponse {
                            status: MatchStatus::Matched,
                            sender_id: Some(event.request.id),
                            receiver_id: Some(our_request.id.clone()),
                            timestamp: (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
                            payload: event.request.payload,
                            message: None,
                        });
                    }
                },
                _ => continue,
            }
        }
        
        Err(BumpError::Timeout)
    }

    #[allow(dead_code)]
    async fn cleanup_expired_requests(&self) -> Result<(), BumpError> {
        // Clean up expired send requests
        self.send_queue.cleanup_expired().await?;
        
        // Clean up expired receive requests
        self.receive_queue.cleanup_expired().await?;
        
        Ok(())
    }

    #[allow(dead_code)]
    fn calculate_distance(&self, loc1: &Location, loc2: &Location) -> f64 {
        // Haversine formula for calculating great-circle distance
        let lat1 = loc1.lat.to_radians();
        let lat2 = loc2.lat.to_radians();
        let delta_lat = (loc2.lat - loc1.lat).to_radians();
        let delta_lon = (loc2.long - loc1.long).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();

        self.config.earth_radius_meters * c
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

}
