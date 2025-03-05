use time::OffsetDateTime;
use crate::error::BumpError;
use crate::queue::types::*;
use crate::queue::matching;
use crate::queue::q_core::UnifiedQueue;

impl UnifiedQueue {
    /// Helper method to find a match for a request in the queue
    /// Returns just the ID of the matched request, not the request itself
    pub(super) fn find_matching_request(&self, request: &QueuedRequest) -> Option<String> {
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
                
                let score = matching::calculate_match_score(
                    request, r,
                    self.max_time_diff_ms,
                    self.max_distance_meters,
                    self.min_score_without_key,
                    self.min_score_with_key,
                    self.custom_key_match_bonus
                );
                
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
    
    /// Helper method to atomically match two requests
    pub(super) fn atomic_match(&self, new_request: &QueuedRequest, matched_request_ref: &QueuedRequest) -> Result<MatchResult, BumpError> {
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
        // Unused, replaced by direct payload exchange below
        let _receive_payload = receive_ref.payload.clone();
        
        // For bump requests, always exchange payloads in both directions
        // For traditional send/receive, only send -> receive gets payload
        let (send_receives, receive_receives) = if new_request.request_type == RequestType::Bump && matched_request_ref.request_type == RequestType::Bump {
            // Bump vs Bump: exchange payloads both ways
            // The issue was here: we need to use the get_request_from_map function to fetch
            // the actual request with payload from the map
            let requests = self.requests.read();
            let map_request_payload = if let Some(req) = requests.get(&matched_request_ref.id) {
                req.payload.clone()
            } else {
                None
            };
            
            // Now exchange the actual payloads
            (map_request_payload, new_request.payload.clone())
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
            
            let (send_request, receive_request) = match (new_request.request_type, map_request.request_type) {
                (RequestType::Bump, RequestType::Bump) => {
                    // For Bump/Bump matches, preserve the channels and payloads carefully
                    // First request (map_request) has its channel preserved by removing from map above
                    // For the second request (new_request), we need its channel from the parameter
                    
                    // Log detailed debug info
                    log::debug!("Bump/Bump match: map_req.id={}, new_req.id={}, existing_in_map={}", 
                              map_request.id, new_request.id, existing_in_map);
                    log::debug!("Map request channel: {}, New request channel: {}", 
                               map_request.response_tx.is_some(), new_request_with_channel.response_tx.is_some());
                    
                    let _map_req_has_channel = map_request.response_tx.is_some();
                    let _new_req_has_channel = new_request_with_channel.response_tx.is_some();
                    
                    // For Bump vs Bump, both sides should get each other's payloads
                    // and preserve their own channels
                    let send = map_request;
                    let receive = new_request_with_channel;
                    
                    // We need to ensure both requests have their original channels
                    // No need to transfer channels between them
                    // Just make sure we don't lose any channels during processing
                    
                    // Log the channel states
                    log::debug!("send (id={}) has channel: {}, receive (id={}) has channel: {}", 
                             send.id, send.response_tx.is_some(), receive.id, receive.response_tx.is_some());
                    
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
            let mut send_request = send_request;
            let mut receive_request = receive_request;
            
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
            match tx {
                ResponseChannel::OneShot(tx) => {
                    if let Err(e) = tx.send(send_match_result.clone()) {
                        log::error!("Failed to send match result to send request {}: {:?}", send_id, e);
                    }
                },
                ResponseChannel::Broadcast(tx) => {
                    if let Err(e) = tx.send(send_match_result.clone()) {
                        log::error!("Failed to broadcast match result to send request {}: {:?}", send_id, e);
                    }
                }
            }
        }
        
        if let Some(tx) = receive_tx {
            log::info!("Notifying receive request {} of match", receive_id);
            match tx {
                ResponseChannel::OneShot(tx) => {
                    if let Err(e) = tx.send(receive_match_result.clone()) {
                        log::error!("Failed to send match result to receive request {}: {:?}", receive_id, e);
                    }
                },
                ResponseChannel::Broadcast(tx) => {
                    if let Err(e) = tx.send(receive_match_result.clone()) {
                        log::error!("Failed to broadcast match result to receive request {}: {:?}", receive_id, e);
                    }
                }
            }
        }
        
        Ok(send_match_result)
    }
}