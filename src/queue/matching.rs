use crate::queue::{QueuedRequest, RequestType, EARTH_RADIUS_METERS};

/// Calculate match score between two requests
/// Returns Some(score) if the requests are compatible and match criteria, None otherwise
pub fn calculate_match_score(
    req1: &QueuedRequest, 
    req2: &QueuedRequest,
    max_time_diff_ms: i64,
    max_distance_meters: f64,
    _min_score_without_key: i32,
    min_score_with_key: i32,
    custom_key_match_bonus: i32
) -> Option<i32> {
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
    
    log::info!("Time diff between {} and {}: {}ms (max allowed: {}ms)", 
              req1.id, req2.id, time_diff, max_time_diff_ms);
    
    // Log the timestamps for debugging
    log::info!("Timestamps: {} = {}, {} = {}", 
             req1.id, req1.matching_data.timestamp,
             req2.id, req2.matching_data.timestamp);
    
    if time_diff > max_time_diff_ms {
        // Time difference too large
        log::debug!("Time difference too large ({}ms) between {} and {}", 
                  time_diff, req1.id, req2.id);
        return None;
    } else {
        // Calculate time proximity score (0-100)
        // 0 diff = 100 points, max_diff = 0 points, linear scale in between
        let time_score = 100 - ((time_diff as f64 / max_time_diff_ms as f64) * 100.0) as i32;
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
        
        log::info!("Comparing locations for {} and {}: Req1(lat:{}, lon:{}) vs Req2(lat:{}, lon:{})", 
                 req1.id, req2.id, loc1.lat, loc1.long, loc2.lat, loc2.long);
        
        // Haversine formula for distance calculation
        let a = (delta_lat / 2.0).sin().powi(2) + 
               lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        
        // Calculate distance using Earth radius constant
        let distance = EARTH_RADIUS_METERS * c;
        
        log::info!("Calculated distance: {:.2}m (Threshold: {:.2}m) between {} and {}", 
                  distance, max_distance_meters, req1.id, req2.id);
        
        if distance <= max_distance_meters {
            // Calculate distance score (0-100)
            // 0 distance = 100 points, max_distance = 0 points
            let distance_score = 100 - ((distance / max_distance_meters) * 100.0) as i32;
            log::debug!("Location match between {} and {}: distance={:.2}m, score={}", 
                      req1.id, req2.id, distance, distance_score);
            log::info!("Calculated distance score: {} for {} and {}", distance_score, req1.id, req2.id);
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
            score += custom_key_match_bonus;
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
        let custom_key_threshold = min_score_with_key / 2;  // Half the regular key threshold
        log::info!("Using low threshold (custom key match): {}", custom_key_threshold);
        custom_key_threshold
    } else {
        // Location match only - use standard threshold
        log::info!("Using standard threshold with location match: {}", min_score_with_key);
        min_score_with_key
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