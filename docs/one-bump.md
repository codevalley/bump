# One-Bump Endpoint Architecture

## Introduction

The current Bump service architecture uses two distinct endpoints (`/send` and `/receive`) with asymmetric behavior for matching and data exchange. This document outlines a simplified approach using a single `/bump` endpoint where clients use identical request structures and matching logic becomes more streamlined.

## Proposed Changes

### New Endpoint: `/bump`

The new endpoint will:

1. Accept a unified request structure containing both matching parameters and an optional payload
2. Add the request to a single unified queue
3. Match with compatible requests based on the same criteria we use now
4. When matched, exchange payloads between both parties
5. Return the same response structure to both sides, containing the counterpart's payload

### Request Structure

```json
{
  "matching_data": {
    "location": {"lat": 37.7749, "long": -122.4194},
    "timestamp": 1714435201000,
    "custom_key": "optional-matching-key"
  },
  "payload": "Optional message or data to share",
  "ttl": 8000
}
```

### Response Structure

```json
{
  "status": "matched",
  "matched_with": "request-id-123",
  "timestamp": 1714435201500,
  "payload": "Payload from the matching request",
  "message": "Optional context information"
}
```

## Integration Strategy

### Should we remove `/send` and `/receive`?

**Recommendation: Keep existing endpoints during transition phase**

1. **API Compatibility**: Many clients may already be using the existing endpoints. Maintaining backward compatibility is important for a smooth transition.

2. **Code Impact**: Our underlying queue and matching systems already handle both request types. The service layer contains most of the differentiation logic, which can be refactored gradually.

3. **Transition Plan**: We should implement the new endpoint alongside the existing ones, then gradually deprecate the old endpoints after clients have time to migrate.

## Refactoring Approach

### Phase 1: Add Unified Endpoint

1. **Create Unified Request/Response Models**:
   - Develop a `BumpRequest` model based on the common elements of `SendRequest` and `ReceiveRequest`
   - Update `MatchResponse` to handle the symmetric payload exchange

2. **Implement `/bump` Endpoint**:
   - Add new handler in `api.rs` for the `/bump` endpoint 
   - Map the unified request to our internal queue representation
   - Reuse the existing matching logic

3. **Adapt Queue Implementation**:
   - Remove the type discrimination in the matching process where appropriate
   - Focus matching on compatible criteria rather than request types

### Phase 2: Refactor Internal Models

1. **Remove Send/Receive Type Distinction**:
   - Update `QueuedRequest` to contain an optional payload without type distinction
   - Modify `RequestType` enum to reflect the unified approach
   - Update matching logic to exchange payloads bidirectionally

2. **Refactor Scoring System**:
   - Simplify match scoring to not consider request types
   - Focus purely on matching criteria quality

### Phase 3: Modernize Existing Endpoints

1. **Refactor `/send` and `/receive` to Use New Logic**:
   - Update handlers to use the new unified request processing
   - Maintain backward compatibility through adapter patterns

2. **Add Deprecation Notices**:
   - Document the planned deprecation timeline
   - Add deprecation headers to API responses

### Phase 4: Complete Migration

1. **Deprecate Old Endpoints**:
   - After sufficient migration period, mark old endpoints as deprecated
   - Consider redirecting traffic to the new endpoint

2. **Remove Legacy Code**:
   - Clean up redundant type checks and send/receive specific logic
   - Simplify codebase by removing duplicate handler code

## Code Structure Impact

The primary affected files will be:

1. **`api.rs`**: Add new endpoint handler, adapt existing handlers
2. **`models.rs`**: Create unified request/response models
3. **`service.rs`**: Refactor service layer to handle unified requests
4. **`queue.rs`**: Simplify matching logic to be bidirectional

The changes should be largely isolated to these areas, with minimal impact on the core matching algorithm and queue management.

## Benefits

1. **Simplified API**: Clients only need to understand one endpoint and one request structure
2. **Reduced Code Complexity**: Removes the artificial distinction between send/receive
3. **More Intuitive Model**: Both sides of a match are treated identically
4. **Easier Testing**: Simplified request structure means easier test setups
5. **More Flexible User Flows**: Clients can include optional payloads regardless of intent

## Challenges

1. **Migration Period**: Need to support both models during transition
2. **Client Updates**: Clients will eventually need to update to the new endpoint
3. **Documentation**: Need to clearly document the transition plan
4. **Race Conditions**: Ensure previous race condition fixes still apply in the new model

## Conclusion

The unified `/bump` endpoint represents a significant architectural improvement that simplifies both the API and internal implementation. By following a phased approach, we can maintain backward compatibility while incrementally moving toward a cleaner, more intuitive design.