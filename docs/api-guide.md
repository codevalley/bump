  # Bump Service API Guide

  This guide explains how to use the Bump service APIs for proximity-based data exchange.

  ## API Overview

  The Bump service provides four main endpoints:

  1. **Health Check** - Check service status and metrics
  2. **Send Data** - Send data for someone nearby to receive
  3. **Receive Data** - Receive data from someone nearby
  4. **Bump** - Unified endpoint to both send and receive data in a single request

  All endpoints accept and return JSON data.

  ## Base URL

  ```
  https://bump-production.up.railway.app
  ```

  ---

  ## 1. Health Check API

  The health check API returns service status and statistics.

  ### Endpoints

  ```
  GET /
  ```

  or

  ```
  GET /bump/health
  ```

  ### Sample Response

  ```json
  {
    "status": "ok",
    "version": "0.2.5",
    "uptime_seconds": 3600,
    "metrics": {
      "send_queue_capacity": 1000,
      "receive_queue_capacity": 1000,
      "cleanup_interval_ms": 1000,
      "max_time_diff_ms": 500,
      "max_distance_meters": 5
    },
    "queue_stats": {
      "send_queue_size": 12,
      "receive_queue_size": 5,
      "matches_count": 256,
      "expired_count": 18,
      "match_rate": 4.27
    }
  }
  ```

  ### Response Fields

  | Field | Description |
  |-------|-------------|
  | `status` | Service status ("ok", "degraded", or "error") |
  | `version` | Current service version |
  | `uptime_seconds` | How long the service has been running |
  | `metrics` | Service configuration values |
  | `queue_stats` | Statistics about the current queue state |

  ---

  ## 2. Send Data API

  The send API allows you to send data that can be received by another client nearby.

  ### Endpoint

  ```
  POST /bump/send
  ```

  ### Request Format

  ```json
  {
    "matching_data": {
      "timestamp": 1647529842000,
      "location": {
        "lat": 37.7749,
        "long": -122.4194
      },
      "custom_key": "optional-secret-key"
    },
    "payload": "The data you want to transfer",
    "ttl": 10000
  }
  ```

  ### Request Fields

  | Field | Description |
  |-------|-------------|
  | `matching_data` | Parameters used to match with nearby receivers |
  | └─ `timestamp` | Current time in milliseconds (use `Date.now()` in JavaScript) |
  | └─ `location` | Geographic coordinates (optional but recommended) |
  | └─└─ `lat` | Latitude between -90 and 90 |
  | └─└─ `long` | Longitude between -180 and 180 |
  | └─ `custom_key` | Secret key to ensure you match with the right person (optional) |
  | `payload` | The data to send (string, can be text, URL, JSON string, etc.) |
  | `ttl` | Time to live in milliseconds (how long to wait for a match) |

  ### Sample Response (Success)

  ```json
  {
    "status": "matched",
    "sender_id": "send_request_123",
    "receiver_id": "receive_request_456",
    "timestamp": 1647529842123,
    "payload": "The data you want to transfer"
  }
  ```

  ### Sample Response (Timeout)

  ```json
  {
    "status": "timeout",
    "timestamp": 1647529852123,
    "message": "No match found within 10000ms"
  }
  ```

  ### Response Fields

  | Field | Description |
  |-------|-------------|
  | `status` | Result of the matching attempt ("matched" or "timeout") |
  | `sender_id` | ID of the sender (your request) |
  | `receiver_id` | ID of the receiver that matched |
  | `timestamp` | When the match occurred |
  | `payload` | The data that was transferred (in send responses) |
  | `message` | Informational message (for timeouts) |

  ---

  ## 3. Receive Data API

  The receive API allows you to receive data from a nearby sender.

  ### Endpoint

  ```
  POST /bump/receive
  ```

  ### Request Format

  ```json
  {
    "matching_data": {
      "timestamp": 1647529842000,
      "location": {
        "lat": 37.7749,
        "long": -122.4194
      },
      "custom_key": "optional-secret-key"
    },
    "ttl": 10000
  }
  ```

  ### Request Fields

  | Field | Description |
  |-------|-------------|
  | `matchingData` | Parameters used to match with nearby senders |
  | └─ `timestamp` | Current time in milliseconds (use `Date.now()` in JavaScript) |
  | └─ `location` | Geographic coordinates (optional but recommended) |
  | └─└─ `lat` | Latitude between -90 and 90 |
  | └─└─ `long` | Longitude between -180 and 180 |
  | └─ `custom_key` | Secret key to ensure you match with the right person (optional) |
  | `ttl` | Time to live in milliseconds (how long to wait for a match) |

  ### Sample Response (Success)

  ```json
  {
    "status": "matched",
    "sender_id": "send_request_123",
    "receiver_id": "receive_request_456",
    "timestamp": 1647529842123,
    "payload": "The data that was sent to you"
  }
  ```

  ### Sample Response (Timeout)

  ```json
  {
    "status": "timeout",
    "timestamp": 1647529852123,
    "message": "No match found within 10000ms"
  }
  ```

  ### Response Fields

  | Field | Description |
  |-------|-------------|
  | `status` | Result of the matching attempt ("matched" or "timeout") |
  | `sender_id` | ID of the sender that matched |
  | `receiver_id` | ID of the receiver (your request) |
  | `timestamp` | When the match occurred |
  | `payload` | The data that was sent to you |
  | `message` | Informational message (for timeouts) |

  ---

  ## Example Usage with JavaScript

  Here's how to use the Bump service with JavaScript fetch API:

  ```javascript
  // Get current position
  navigator.geolocation.getCurrentPosition(async (position) => {
    // 1. Send data
    const sendResponse = await fetch('https://bump-production.up.railway.app/bump/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        matching_data: {
          timestamp: Date.now(),
          location: {
            lat: position.coords.latitude,
            long: position.coords.longitude
          },
          custom_key: "my-secret-key-123"
        },
        payload: "Hello from sender!",
        ttl: 10000 // Wait up to 10 seconds for a match
      })
    });
    
    const sendResult = await sendResponse.json();
    console.log("Send result:", sendResult);
    
    // 2. Receive data
    const receiveResponse = await fetch('https://bump-production.up.railway.app/bump/receive', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        matching_data: {
          timestamp: Date.now(),
          location: {
            lat: position.coords.latitude,
            long: position.coords.longitude
          },
          custom_key: "my-secret-key-123"
        },
        ttl: 10000 // Wait up to 10 seconds for a match
      })
    });
    
    const receiveResult = await receiveResponse.json();
    console.log("Receive result:", receiveResult);
  });
  ```

  ## Matching Algorithm

  The Bump service uses a sophisticated matching algorithm with the following characteristics:

  - **Temporal Proximity**: Matches requests that occur within a short time window (default: 500ms)
  - **Spatial Proximity**: If location is provided, matches nearby requests (default: within 5 meters)
  - **Custom Keys**: Optionally uses exact key matching for increased security
  - **Weighted Scoring**: Uses a weighted scoring system to find the best match
  - **TTL-based Expiration**: Requests expire after their TTL and can no longer be matched

  ## Tips for Successful Matching

  1. **Use Accurate Timestamps**: Always use the current time in milliseconds
  2. **Provide Location**: While optional, providing location improves matching accuracy
  3. **Custom Keys**: Use custom keys when you want to ensure you match with a specific user
  4. **Appropriate TTL**: Choose a TTL value that balances:
    - User patience (how long they're willing to wait)
    - Expected proximity (physically closer users match faster)
    - Network conditions (slower networks need longer TTLs)
  5. **Handle Timeouts**: Always implement timeout handling in your client

  ## 4. Bump API (Unified Endpoint)

  The `/bump` endpoint provides a unified way to both send and receive data in a single request.

  ### Endpoint

  ```
  POST /bump
  ```

  ### Request Format

  ```json
  {
    "matching_data": {
      "timestamp": 1714435201000,
      "location": {
        "lat": 37.7749, 
        "long": -122.4194
      },
      "custom_key": "optional-matching-key"
    },
    "payload": "Optional message or data to share",
    "ttl": 500
  }
  ```

  ### Request Fields

  | Field | Type | Required | Description |
  |-------|------|----------|-------------|
  | `matching_data.timestamp` | number | Yes | Current time in milliseconds since epoch |
  | `matching_data.location` | object | No | Geographic coordinates for proximity matching |
  | `matching_data.location.lat` | number | - | Latitude in decimal degrees |
  | `matching_data.location.long` | number | - | Longitude in decimal degrees |
  | `matching_data.custom_key` | string | No | Optional identifier for exact matching |
  | `payload` | any | No | Data to share with the matched request |
  | `ttl` | number | No | Time to live in milliseconds (default: 500ms) |

  ### Sample Response (Success)

  ```json
  {
    "status": "matched",
    "sender_id": "request-id-456",
    "receiver_id": "request-id-123",
    "timestamp": 1714435201500,
    "payload": "Payload from the matching request",
    "message": "Match successful"
  }
  ```

  ### Sample Response (Timeout)

  ```json
  {
    "status": "timeout",
    "timestamp": 1714435210000,
    "message": "No match found within specified TTL"
  }
  ```

  ### Response Fields

  | Field | Type | Description |
  |-------|------|-------------|
  | `status` | string | Result of the request ("matched" or "timeout") |
  | `sender_id` | string | ID of your request |
  | `receiver_id` | string | ID of the matched request |
  | `timestamp` | number | When the match occurred (milliseconds since epoch) |
  | `payload` | any | Data received from the matched request |
  | `message` | string | Optional informational message |

  ### Example Usage with JavaScript

  ```javascript
  // Get current position
  navigator.geolocation.getCurrentPosition(async (position) => {
    const bumpResponse = await fetch('https://bump-production.up.railway.app/bump', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        matching_data: {
          timestamp: Date.now(),
          location: {
            lat: position.coords.latitude,
            long: position.coords.longitude
          },
          custom_key: "room-123"
        },
        payload: "Hello from client A!",
        ttl: 1000 // Wait up to 1 second for a match
      })
    });
    
    const result = await bumpResponse.json();
    console.log("Bump result:", result);
    
    if (result.status === "matched") {
      console.log("Received payload:", result.payload);
    }
  });
  ```

  ### Benefits of Using the Unified Endpoint

  1. **Simplified Implementation**: Use a single endpoint for both sending and receiving data
  2. **Reduced Requests**: Complete an exchange with just one request per client
  3. **Symmetric Behavior**: Both sides of a match are treated identically
  4. **Flexible Data Exchange**: Both parties can optionally send data in the same transaction

  ---

  ## Common Errors

  | Status Code | Description | Solution |
  |-------------|-------------|----------|
  | 429 Too Many Requests | Queue is full | Try again later with exponential backoff |
  | 408 Request Timeout | No match found within TTL | Increase TTL or try again |
  | 400 Bad Request | Invalid request format | Check request format and parameters |
  | 500 Internal Server Error | Server issue | Check the service status and try again later |