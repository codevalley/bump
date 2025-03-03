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

## Race Conditions and Two-Phase Matching

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

### Two-Phase Matching Solution

To fix this race condition, we implemented an updated matching protocol:

1. **Improved Queue Management**
   - The matching algorithm maintains precise control over queue state
   - Synchronized request removal to prevent race conditions
   - Improved notification system for match events

2. **Atomic Operations**
   - Uses atomic operations to prevent multiple simultaneous matches
   - Carefully orchestrated state management
   - Clear notification to both parties  

This approach ensures that:
- No request is removed until both sides are ready
- No request can be matched multiple times
- System remains consistent even under high concurrency

### Implementation Details

The matching solution uses:
- Atomic queue operations
- Synchronized request processing
- Strong ordering guarantees
- Timeout mechanism for incomplete matches
- Rollback procedure for failed matches

### Error Handling

The matching protocol gracefully handles several edge cases:
- Timeouts during the matching process
- Concurrent matching attempts 
- System failures during any phase
- Race conditions with multiple clients

This robust approach ensures that the matching service maintains consistency even under high load and concurrent operations.

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