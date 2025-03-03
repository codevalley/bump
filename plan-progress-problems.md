# Bump Service Development Plan

## Plan

### Phase 1: Initial Setup and Core API
1. Set up project structure and dependencies
   - Create minimal `Cargo.toml` with essential dependencies
   - Set up directory structure following Rust best practices
   - Configure logging and error handling

2. Implement Core Types and Structures
   - Define data models for request/response types
   - Implement matching algorithm types
   - Create error types and handling utilities

3. Create API Server
   - Set up Actix-web server (chosen for performance and ease of use)
   - Implement `/bump/send` and `/bump/receive` endpoints
   - Add basic request validation

4. Implement Matching Service
   - Create in-memory request pool
   - Implement matching algorithm with configurable tolerances
   - Add timeout handling

### Phase 2: Testing and Documentation
1. Unit Tests
   - Test matching algorithm
   - Test request/response handling
   - Test timeout scenarios

2. Integration Tests
   - End-to-end API tests
   - Concurrent request handling tests
   - Performance benchmarks

3. Documentation
   - API documentation
   - Code documentation
   - Setup and deployment guide

### Phase 3: Performance Optimization
1. Performance Testing
   - Load testing
   - Memory usage analysis
   - Response time optimization

2. Security Hardening
   - Rate limiting
   - Input validation
   - HTTPS support

## Progress

### Current Status
- Initial project planning complete
- Project structure established with modular design
- Core matching service implemented with:
  - New matching logic with mandatory time proximity
  - Optional but weighted location and custom key matching
  - Request validation for matching criteria
- Configuration system with environment variable support
- Periodic cleanup of expired requests
- ✅ Queue system redesigned with event-based architecture:
  - Implemented trait-based queue interface
  - Added in-memory implementation using broadcast channels
  - Real-time event notifications for matches
  - Efficient request cleanup
  - Support for future Redis implementation
- **Current Challenge**: Implementing comprehensive test suite

## Problems and Learnings

### Design Evolution
1. **Initial Queue Implementation (Current)**:
   - Using HashMap with RwLock for thread safety
   - Polling-based matching strategy (every 50ms)
   - Limitations:
     - Inefficient active polling
     - No real-time notification system
     - Not easily extensible for different storage backends

2. **Proposed Queue Redesign**:
   - Create an abstract Queue trait for send/receive operations
   - Use channels for real-time notifications
   - Support for different implementations (memory, Redis, etc.)
   - Better separation of concerns

### Technical Decisions
1. **Choice of Web Framework**: Actix-web
   - Pros: High performance, async support, mature ecosystem
   - Cons: Slightly more complex than alternatives like Rocket
   - Decision: Performance is a key requirement, making Actix-web the better choice

2. **In-Memory Storage & Concurrency**:
   - Implemented using HashMap with parking_lot::RwLock
   - Chose parking_lot over std::sync for better performance
   - Optimized lock patterns to minimize contention
   - Added efficient cleanup mechanism for expired requests

3. **Matching Algorithm Design**:
   - Implemented weighted scoring (70% temporal, 30% spatial)
   - Used Haversine formula for accurate distance calculation
   - Added early filtering to reduce computation load
   - Support for optional custom keys as additional verification

### Queue Design Considerations
1. **Interface Requirements**:
   - Async-first design
   - Support for timeout-based operations
   - Efficient matching criteria evaluation
   - Clear ownership and lifecycle management

2. **Implementation Options**:
   - tokio::sync::broadcast for notifications
   - Custom channel implementation for complex matching
   - Potential for distributed queue support

### Anticipated Challenges
1. **Concurrency Handling**
   - Need careful design of the request pool to handle concurrent access
   - Must ensure thread safety without sacrificing performance

2. **Timeout Management**
   - Efficient cleanup of timed-out requests
   - Balancing between memory usage and performance

3. **Location Matching**
   - Efficient spatial indexing for location-based matching
   - Handling edge cases in location accuracy

### Test Plan

#### 1. Unit Tests

##### Queue Implementation
- Test queue operations (add, remove, find)
- Verify event broadcasting
- Test cleanup of expired requests
- Validate thread safety
- Test error conditions

##### Matching Service
- Test send request processing
  - Immediate matches
  - Delayed matches
  - Timeouts
  - Error cases
- Test receive request processing
  - Immediate matches
  - Delayed matches
  - Timeouts
  - Error cases
- Test matching algorithm
  - Time proximity scoring
  - Location proximity scoring
  - Custom key matching
  - Combined criteria scoring

##### Configuration
- Test environment variable loading
- Validate default values
- Test invalid configurations

#### 2. Integration Tests

##### API Endpoints
- Test `/bump/send` endpoint
  - Valid requests
  - Invalid requests
  - Timeout scenarios
- Test `/bump/receive` endpoint
  - Valid requests
  - Invalid requests
  - Timeout scenarios

##### Concurrent Operations
- Test multiple simultaneous send requests
- Test multiple simultaneous receive requests
- Test mixed send/receive scenarios
- Verify thread safety under load

##### Error Handling
- Test invalid input handling
- Test timeout handling
- Test system error recovery

#### 3. Performance Tests

##### Load Testing
- Measure request throughput
- Test memory usage under load
- Verify cleanup effectiveness
- Test concurrent request handling

##### Latency Testing
- Measure matching latency
- Test event propagation speed
- Verify timeout accuracy

##### Resource Usage
- Monitor CPU usage
- Track memory allocation
- Measure lock contention

### Next Steps
1. Implement test suite following the above plan
2. Add performance benchmarks
3. Consider additional features:
   - Rate limiting
   - Metrics and monitoring
   - Request validation middleware
   - Redis queue implementation

*Note: This document will be updated as development progresses.*
