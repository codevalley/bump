# Bump Service Technical Specification

## 1. Introduction

### 1.1 Problem Statement
Mobile devices lack a simple, intuitive way to exchange data when in physical proximity. Traditional methods like QR codes, NFC, or Bluetooth require specific hardware support or complex setup procedures. Users want a natural way to share content that mirrors the physical action of "passing" data from one device to another.

### 1.2 Solution Overview
The Bump Service enables data exchange between two devices through a simple, physical gesture: bumping the devices together. The service uses timestamp, location data, and an optional custom key to identify when two devices have been physically bumped against each other, then facilitates the data transfer between them.

### 1.3 Key Features
- Simple unified API with a single `/bump` endpoint
- Stateless, ephemeral architecture with no persistent storage requirements
- Matching based on location, timestamp, and custom verification key
- Support for text, JSON, or URL payloads
- Synchronous request model with configurable timeouts

## 2. System Architecture

### 2.1 High-Level Overview
The Bump Service operates as a lightweight API service that temporarily holds pending bump requests in memory and matches them based on specified criteria. The service is designed to be stateless, where any failed matches result in request timeouts rather than persisted error states.

### 2.2 Sequence Diagram

```
Device A                 Server                  Device B
   |                       |                       |
   |------ /bump -------->|                       |
   |                      |<------ /bump ---------|
   |                      |                       |
   |                      |---(match algorithm)---|
   |                      |                       |
   |<---- payload B ------|                       |
   |                      |------ payload A ----->|
   |                      |                       |
```

If no match is found within the timeout period:

```
Device A                 Server
   |                       |
   |------ /bump -------->|
   |      (waiting)       |
   |      (timeout)       |
   |<-- timeout response -|
```

### 2.3 Component Descriptions
- **API Layer**: Handles incoming requests and manages connection timeouts
- **Matching Service**: In-memory component that implements the matching algorithm
- **Request Pool**: Temporary storage for pending bump requests
- **Response Handler**: Returns appropriate responses based on match results

## 3. API Specification

### 3.1 Unified Bump Endpoint

```
POST /bump
{
  "matching_data": {
    "location": {"lat": 37.7749, "long": -122.4194},
    "timestamp": 1646078423000,
    "custom_key": "optional-key"
  },
  "payload": "https://example.com/shared-document", // Optional
  "ttl": 500 // Optional, milliseconds to wait for match
}

Response (Success):
{
  "status": "matched",
  "matched_with": "request-id-123",
  "timestamp": 1646078425000,
  "payload": "payload from matching request",
  "message": "Match successful"
}

Response (Timeout):
{
  "status": "timeout",
  "message": "No matching bump request found within the specified time window"
}

Response (Queue Full):
{
  "status": "error",
  "error": "queue_full",
  "message": "Request queue is full, please try again later"
}
```

## 4. Matching Algorithm

### 4.1 Match Criteria
The matching service identifies potential bumps using the following criteria:

1. **Temporal proximity**: Timestamps must be within a configurable tolerance window (default: 500ms)
2. **Spatial proximity**: Locations must be within a configurable radius (default: 5 meters)
3. **Custom key match**: If provided, the customKey must match exactly between both requests

### 4.2 Multiple Match Resolution
In cases where multiple potential matches are found:

1. The exact customKey match takes precedence
2. If still ambiguous, the closest temporal match is selected
3. If still ambiguous, the closest spatial match is selected

### 4.3 Implementation Considerations
- The matching algorithm is intentionally "black boxed" to allow for future improvements
- Tolerances for matching should be configurable server-side
- Performance optimization should focus on rapid matching rather than persistence

## 5. Security and Privacy Considerations

### 5.1 Data Handling
- Location data is used only for matching and not stored after request completion
- No user identification is required beyond what clients choose to include in payload
- All data is ephemeral and not persisted after request completion or timeout

### 5.2 Potential Vulnerabilities
- **Interception**: Without encryption, payloads could be intercepted
- **Spoofing**: Someone could attempt to guess the matching criteria
- **Denial of Service**: Flooding the service with fake bump requests

### 5.3 Mitigations
- Use HTTPS for all API communications
- Implement rate limiting per client IP
- The customKey adds a layer of verification that's difficult to guess
- Consider adding request signing for client verification in production

## 6. Scalability and Performance

### 6.1 Resource Requirements
- Memory usage will scale with concurrent pending requests (limited by max_queue_size)
- CPU usage will spike during matching operations
- Network I/O will be the primary bottleneck for most deployments
- Queue has a configurable maximum size (default: 1000)
- Requests exceeding queue size will receive HTTP 429 (Too Many Requests)

### 6.2 Scaling Strategies
- Horizontal scaling with load balancing is recommended
- Consider using sticky sessions to keep matching requests on the same server
- In-memory data structure should be optimized for fast lookups by location

### 6.3 Performance Targets
- Request matching should complete in <50ms
- Total request lifetime (including network) should be <500ms
- Service should support at least 1000 concurrent pending requests per instance

## 7. Implementation Roadmap

### 7.1 Phase 1: Core Service ✅
- Basic API implementation with in-memory request pool
- Simple matching algorithm based on timestamp and location
- Basic error handling and timeouts

### 7.2 Phase 2: Enhancements ✅
- Add customKey verification
- Improve matching algorithm with configurable tolerances
- Add basic rate limiting and security measures

### 7.3 Phase 3: Optimizations ✅
- Performance tuning for high-concurrency scenarios
- Add metrics and monitoring
- Implement more sophisticated matching algorithms

### 7.4 Phase 4: API Unification ✅
- Merge send/receive endpoints into unified `/bump` endpoint
- Simplify request/response models
- Update documentation and client libraries

## 8. Limitations and Future Considerations

### 8.1 Known Limitations
- GPS accuracy can vary significantly, especially indoors
- Device clock synchronization can affect timestamp matching
- Service restarts will clear all pending requests

### 8.2 Future Enhancements
- Add optional push notifications for asynchronous matching
- Consider add-ons like persistent history for enterprise customers
- Explore additional matching factors (device motion, accelerometer data)
- Implement client SDKs for popular platforms

## 9. Conclusion

The Bump Service provides a simple, intuitive way for devices to exchange data through a physical bumping gesture. By focusing on a minimal API surface with a single unified endpoint and stateless architecture, the service can be easily implemented, deployed, and scaled. The flexible matching algorithm and optional customKey verification provide a balance of user convenience and accuracy.

This specification outlines the core functionality required to implement a viable Bump Service while leaving room for future optimizations and enhancements based on real-world usage patterns and feedback.