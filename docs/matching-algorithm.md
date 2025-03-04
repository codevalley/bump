# Bump Service Matching Algorithm

This document outlines the sophisticated matching algorithm used by the Bump service to pair devices based on proximity data.

## Overview

The Bump service uses a multi-faceted, score-based approach to match "send" and "receive" requests. Instead of simple exact matching, we use a weighted scoring system that accounts for real-world variability in timing, location accuracy, and user behavior.

## Matching Criteria

### 1. Basic Requirements

For two requests to be considered for matching:

- One must be a "send" request (with payload) and one must be a "receive" request (without payload)
- The requests cannot be from the same device (different request IDs)
- Timestamps must be within a configurable maximum time difference
- If both requests have custom keys, they must match exactly

### 2. Scoring Factors

Once basic requirements are met, we calculate a match score based on:

#### Temporal Proximity
- Time difference between request timestamps
- Scored inversely proportional (closer = higher score)
- Maximum time difference: **500ms** (configurable)
- Score range: 0-100 points

#### Spatial Proximity
- Physical distance between request locations (if available)
- Calculated using Haversine formula with Earth radius: **6,371,000 meters**
- Maximum matching distance: **5 meters** (configurable)
- Score range: 0-100 points

#### Custom Key Matching
- Optional client-provided keys for explicit matching
- Exact match required if both sides provide keys
- Non-matching keys immediately disqualify a match
- Matching custom keys add **200 points** to the score

## Threshold-Based Decision

The final decision uses different thresholds based on the matching context:

1. **With custom key match:**
   - Lower threshold: **100 points**
   - Custom key provides strong confidence

2. **Without custom key match:**
   - Higher threshold: **150 points**
   - Requires stronger evidence from time/location

## Implementation Notes

### Configuration Options

The matching algorithm has the following configurable parameters:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_time_diff_ms` | 500 | Maximum time difference (milliseconds) |
| `max_distance_meters` | 5.0 | Maximum location distance (meters) |
| `min_score_without_key` | 150 | Minimum score threshold without custom key |
| `min_score_with_key` | 100 | Minimum score threshold with custom key |
| `key_match_threshold` | 250 | Threshold for considering a key-boosted match |

### Performance Considerations

- The matching algorithm is O(n) with respect to queue size
- All calculations performed in-memory
- Earth radius constant value: 6,371,000 meters

### Security Considerations

- Custom keys can be used for secure/private matching
- Non-matching keys are strictly rejected
- No persistent storage of matching data

## Examples

### Example 1: Perfect Match
- Send request at 12:00:00.000, location (37.7749, -122.4194), key "ABC123"
- Receive request at 12:00:00.000, location (37.7749, -122.4194), key "ABC123"
- Score: 300 points (100 time + 100 location + 200 custom key)
- Result: MATCH (above 100 threshold with key)

### Example 2: Good Match Without Key
- Send request at 12:00:00.100, location (37.7749, -122.4194), no key
- Receive request at 12:00:00.000, location (37.7748, -122.4193), no key
- Score: 180 points (80 time + 100 location)
- Result: MATCH (above 150 threshold without key)

### Example 3: Near Miss
- Send request at 12:00:00.300, location (37.7749, -122.4194), no key
- Receive request at 12:00:00.000, location (37.7749, -122.4194), no key
- Score: 140 points (40 time + 100 location)
- Result: NO MATCH (below 150 threshold without key)

### Example 4: Key Mismatch
- Send request at 12:00:00.000, location (37.7749, -122.4194), key "ABC123"
- Receive request at 12:00:00.000, location (37.7749, -122.4194), key "XYZ789"
- Score: N/A (immediate rejection due to key mismatch)
- Result: NO MATCH

## Race Conditions and Unified Queue Solution

### The Race Condition Problem

The initial implementation of the matching algorithm had a subtle race condition that could lead to "stuck" requests. Here's how it manifested:

1. **When Send Request Arrives First:**
   ```
   Send Request                 Receive Request
   1. Check receive queue      
   2. No match found          
   3. Add to send queue       
   4. Wait for match...       1. Check send queue
                              2. Find match
                              3. Remove send request
                              4. Return success
   5. Still waiting...        (Done)
   ```

2. **When Receive Request Arrives First:**
   ```
   Receive Request             Send Request
   1. Check send queue        
   2. No match found         
   3. Add to receive queue    
   4. Wait for match...      1. Check receive queue
                             2. Find match
                             3. Remove receive request
                             4. Return success
   5. Still waiting...       (Done)
   ```

The core issue was that the matching process would remove a request from its queue before confirming that both sides of the match were ready to complete. This could leave one side perpetually waiting for a match that was already consumed.

### Unified Queue Solution

To fix this race condition, we completely redesigned our approach with a unified queue system:

1. **Single Queue Architecture**
   - One unified queue for both send and receive requests
   - Each request is tagged with a `RequestType` enum (Send/Receive)
   - All matching logic happens in a single consistent flow

2. **Consistent Matching Process**
   ```
   Request (Send or Receive)
   1. Add request to unified queue
   2. Queue immediately checks for matching partners
   3. If match found:
      - Both requests are atomically matched and removed
      - Both requesters are directly notified via oneshot channels
   4. If no match found:
      - Request remains in queue
      - Waiter subscribes to events specific to their request
   ```

3. **Direct Notification System**
   - Each request contains a oneshot channel for direct notification
   - When a match occurs, both sides are notified directly
   - No reliance on broadcast events that could be missed or misinterpreted

### Implementation Benefits

The unified queue approach provides several key advantages:

1. **Simplified Logic**
   - Single code path for matching logic
   - Matching happens exactly once at a well-defined point
   - Clear and consistent flow for all request types

2. **Direct Communication**
   - Each request gets notified directly when matched
   - No need to interpret broadcast events
   - Immediate response when a match is found

3. **Atomic Operations**
   - Match operations happen atomically
   - Both requests are only removed when the match is complete
   - No possibility of "half-matched" states

4. **Race Condition Elimination**
   - All matching decisions happen in a single, locked context
   - Direct notification ensures no requests are left waiting
   - No duplicate match attempts in different code paths

### Channel Management and Race Condition Fix

A critical aspect of our implementation is proper management of the oneshot channels used for notification:

1. **Early Channel Creation**
   - Channels are created before requests enter the queue
   - Each request has its channel from the start
   - No race condition between matching and channel setup

2. **Careful Clone Handling**
   - `QueuedRequest::Clone` implementation sets `response_tx: None`
   - Original request with channel is stored in the queue
   - Events use clones without channels to avoid ownership issues

3. **Atomic Channel Operations**
   - During matching, both channels are extracted within the same lock
   - Channels are taken from the queue before requests are removed
   - Notifications happen after the lock is released

4. **Lock Management**
   - Queue locks are held for minimal duration
   - Notification happens outside critical sections
   - Prevents deadlocks and improves throughput

This careful handling of channels ensures that even when requests arrive sequentially (rather than simultaneously), they will properly match without getting stuck in a waiting state or experiencing a premature timeout.

This robust approach ensures that the matching service maintains consistency even under high load and concurrent operations while eliminating the race conditions that could cause indefinite waiting.

## Future Enhancements

Potential improvements to the matching algorithm:

1. **Machine Learning Integration**
   - Train models on successful matches to optimize thresholds
   - Consider device types, historical patterns, etc.

2. **Device-Specific Adjustments**
   - Adjust thresholds based on device capabilities
   - Higher tolerance for devices with less accurate sensors

3. **Distributed Matching**
   - Scale algorithm to work across multiple service instances
   - Maintain consistency in distributed environments

4. **Meta-Matching**
   - Factor in the number of potential matches
   - In crowded areas, be more selective with threshold

5. **User Feedback Loop**
   - Incorporate success/fail feedback from users
   - Adjust weights and thresholds based on real-world use