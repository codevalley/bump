use tokio::sync::{oneshot, broadcast};
use time::OffsetDateTime;
use crate::error::BumpError;
use crate::queue::types::*;
use crate::queue::q_core::UnifiedQueue;

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
    /// Applications should now create a channel when adding the request and wait on that channel directly.
    async fn wait_for_match(&self, request_id: &str, ttl_ms: u64) -> Result<Option<MatchResult>, BumpError> {
        log::warn!("wait_for_match() is deprecated and may cause race conditions. Requests should include a channel when created.");
        
        // Create a channel for the match result
        let (tx, rx) = oneshot::channel();
        
        // Add the response channel to the request if it exists
        {
            let mut requests = self.requests.write();
            if let Some(request) = requests.get_mut(request_id) {
                request.response_tx = Some(ResponseChannel::OneShot(tx));
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